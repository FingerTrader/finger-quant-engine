// Application-owned standard-library service. Widgets subscribe; they do not own the peer.
import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'client.dart';

class RunView {
  final String handle;
  final String operation;
  final String phase;
  final int cursor;
  final String? resultId;
  final String? errorCode;
  final String? processedBars;
  final bool stopRequested;
  const RunView(this.handle, this.operation, this.phase, this.cursor,
      this.resultId, this.errorCode, this.processedBars, this.stopRequested);
  bool get terminal => ['completed', 'cancelled', 'failed', 'released', 'interrupted'].contains(phase);
}
int _counter(dynamic value, int max) {
  if (value is! String) throw StateError('counter must be a string');
  final n = int.tryParse(value);
  if (n == null || n < 0 || n > max || '$n' != value) throw StateError('counter range/encoding');
  return n;
}
class ManagedClient {
  final PipeClient pipe;
  final String session;
  final _views = <String, RunView>{};
  final _refreshing = <String, Future<RunView>>{};
  final _events = StreamController<RunView>.broadcast();
  bool _closed = false;
  ManagedClient._(this.pipe, this.session);
  Stream<RunView> get updates => _events.stream;
  Map<String, RunView> get runs => Map.unmodifiable(_views);
  static Future<ManagedClient> start(String executable) async {
    final pipe = await PipeClient.start(executable);
    try {
      final hello = await pipe.hello();
      if (!(hello['capabilities'] as List).contains('run.managed/1')) throw StateError('managed runs unsupported');
      final init = await pipe.call('run.initialize', {});
      if (init['session_id'] is! String || init['session_id'] != hello['session_id'] ||
          init['max_jobs'] != '4' || init['max_operations'] != '64' ||
          init['max_active'] != '1' || init['progress'] != 'indeterminate-batch/1' ||
          init['event_delivery'] != 'pull-cursor/1') throw StateError('managed contract');
      return ManagedClient._(pipe, init['session_id'] as String);
    } catch (_) { await pipe.close(); rethrow; }
  }
  void _publish(RunView view) {
    final old = _views[view.handle];
    if (old != null) {
      if (['released', 'interrupted'].contains(old.phase) && old.phase != view.phase) return;
      if (old.phase == view.phase && old.cursor == view.cursor && old.resultId == view.resultId &&
          old.errorCode == view.errorCode && old.processedBars == view.processedBars &&
          old.stopRequested == view.stopRequested) return;
    }
    _views[view.handle] = view;
    if (!_events.isClosed) _events.add(view);
  }
  Future<Map<String, dynamic>> _call(String method, Map<String, dynamic> params) async {
    if (_closed) throw StateError('service closed');
    try { return await pipe.call(method, {'session_id': session, ...params}); }
    catch (_) {
      if (pipe.disconnected) {
        for (final view in _views.values.toList()) {
          if (!view.terminal) _publish(RunView(view.handle, view.operation, 'interrupted',
            view.cursor, null, 'disconnected', view.processedBars, view.stopRequested));
        }
      }
      rethrow;
    }
  }
  Future<RunView> startRun(String operation, Map<String, dynamic> input) async {
    // Snapshot the draft before the first await. Later edits cannot alter this request.
    final frozen = jsonDecode(jsonEncode(input)) as Map<String, dynamic>;
    final reply = await _call('run.start', {'operation_id': operation, 'input': frozen});
    final handle = reply['handle'];
    if (reply['session_id'] != session || handle is! String ||
        reply['duplicate'] is! bool || reply['released'] is! bool) throw StateError('start reply');
    if (_views.containsKey(handle)) return _views[handle]!;
    final view = RunView(handle, operation, reply['released'] == true ? 'released' : 'accepted',
      0, null, null, null, false);
    _publish(view); return _views[handle]!;
  }
  Future<RunView> refresh(String handle) {
    final current = _refreshing[handle]; if (current != null) return current;
    final future = _refresh(handle); _refreshing[handle] = future;
    // Attach both branches so a failure never leaves an unhandled derived future.
    future.then((_) { _refreshing.remove(handle); }, onError: (Object _, StackTrace __) { _refreshing.remove(handle); });
    return future;
  }
  Future<RunView> _refresh(String handle) async {
    final old = _views[handle]; if (old == null) throw StateError('unknown client run');
    if (old.phase == 'released' || old.phase == 'interrupted') return old;
    final result = await _call('run.events', {'handle': handle, 'after': '${old.cursor}', 'limit': '16'});
    final next = _counter(result['next_cursor'], 4);
    final high = _counter(result['high_watermark'], 4);
    final rows = result['events'];
    if (result['session_id'] != session || result['handle'] != handle || rows is! List ||
        next < old.cursor || next > high || rows.length != next - old.cursor || result['terminal'] is! bool) throw StateError('event cursor');
    var phase = old.phase; var cursor = old.cursor;
    for (final row in rows) {
      if (row is! Map || _counter(row['cursor'], 4) != ++cursor ||
          !['accepted', 'running', 'cancelling', 'completed', 'cancelled', 'failed'].contains(row['phase'])) throw StateError('event sequence');
      phase = row['phase'] as String;
    }
    final status = await _call('run.status', {'handle': handle});
    // Status can be newer than this event page. Only the consumed event determines the displayed phase.
    final terminal = ['completed', 'cancelled', 'failed'].contains(phase);
    if (terminal && status['phase'] != phase) throw StateError('terminal rewrite');
    final view = RunView(handle, old.operation, phase, next,
      terminal ? status['result_id'] as String? : null,
      terminal && status['error'] is Map ? status['error']['code'] as String? : null,
      terminal ? status['processed_source_bars'] as String? : null,
      status['stop_requested'] == true);
    _publish(view); return _views[handle]!;
  }
  Future<RunView> wait(String handle, {Duration deadline = const Duration(seconds: 5)}) async {
    final watch = Stopwatch()..start();
    while (true) {
      final view = await refresh(handle); if (view.terminal) return view;
      if (watch.elapsed > deadline) throw TimeoutException('managed terminal deadline');
      await Future<void>.delayed(const Duration(milliseconds: 5));
    }
  }
  Future<void> stop(String handle) async { await _call('run.stop', {'handle': handle}); }
  Future<Map<String, dynamic>> result(String handle) => _call('run.result', {'handle': handle});
  Future<Uint8List> resultBytes(String handle) async {
    final view = await wait(handle);
    if (view.resultId == null) throw StateError('no financial result: ${view.phase}');
    final bytes = BytesBuilder(copy: false); var offset = 0;
    while (true) {
      final chunk = await _call('run.result_chunk', {'handle': handle, 'result_id': view.resultId,
        'offset': '$offset', 'max_bytes': '65536'});
      final next = _counter(chunk['next_offset'], maxResponse);
      final total = _counter(chunk['total_bytes'], maxResponse);
      final hex = chunk['data'];
      if (chunk['result_id'] != view.resultId || chunk['encoding'] != 'hex' ||
          _counter(chunk['offset'], maxResponse) != offset || next < offset || next > total ||
          next - offset > 65536 || hex is! String || hex.length != (next - offset) * 2 ||
          !RegExp(r'^[0-9a-f]*$').hasMatch(hex) || chunk['eof'] != (next == total) ||
          (next == offset && next != total)) throw StateError('chunk cursor/value');
      final decoded = Uint8List(next - offset);
      for (var i = 0; i < decoded.length; i++) { decoded[i] = int.parse(hex.substring(i * 2, i * 2 + 2), radix: 16); }
      bytes.add(decoded); offset = next;
      if (next == total) return bytes.takeBytes();
    }
  }
  Future<Map<String, dynamic>> output(String handle, String section, int after, {int limit = 16}) async {
    final view = await wait(handle);
    if (view.resultId == null) throw StateError('no financial result');
    return _call('run.output', {'handle': handle, 'result_id': view.resultId,
      'section': section, 'after': '$after', 'limit': '$limit'});
  }
  Future<void> release(String handle) async {
    await _call('run.release', {'handle': handle}); final old = _views[handle];
    if (old != null) _publish(RunView(handle, old.operation, 'released', old.cursor, null, null, old.processedBars, old.stopRequested));
  }
  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    // Pipe EOF requests native cancellation; transport bounds graceful exit then forced cleanup.
    await pipe.close();
    for (final old in _views.values.toList()) {
      if (!old.terminal) _publish(RunView(old.handle, old.operation, 'interrupted', old.cursor, null, 'service_closed', old.processedBars, old.stopRequested));
    }
    // A paused widget subscription cannot hold native process teardown open.
    unawaited(_events.close());
  }
}
