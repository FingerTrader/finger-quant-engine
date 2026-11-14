// First-party experimental client; Dart SDK standard libraries only. No Flutter/Pub.
import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

const maxRequest = 262144;
const maxResponse = 4 * 1024 * 1024;
const requiredCapabilities = ['desktop.exact-json/1', 'backtest.cash-long/1'];

Map<String, dynamic> request(String id, String method, Map<String, dynamic> params) => {
  'format': 'fqe.desktop-request', 'schema_version': 1,
  'id': id, 'method': method, 'params': params,
};
Map<String, dynamic> decodeResponse(List<int> bytes, String id) {
  if (bytes.isEmpty || bytes.length > maxResponse) throw StateError('response quota');
  final value = jsonDecode(utf8.decode(bytes));
  if (value is! Map<String, dynamic> || value.length != 5 ||
      value['format'] != 'fqe.desktop-response' || value['schema_version'] != 1 ||
      value['id'] != id || value['ok'] is! bool || value['body'] is! Map<String, dynamic>) {
    throw StateError('response protocol/correlation');
  }
  if (value['ok'] != true) throw StateError('engine: ${value['body']['code']}');
  return value['body'] as Map<String, dynamic>;
}
void checkHello(Map<String, dynamic> body) {
  final caps = body['capabilities'];
  if (body['major'] != 1 || body['minor'] != 0 || caps is! List ||
      requiredCapabilities.any((c) => !caps.contains(c)) ||
      body['max_request_bytes'] != '$maxRequest' ||
      body['max_response_bytes'] != '$maxResponse' ||
      body['financial_authority'] != 'native-ledger') {
    throw StateError('incompatible engine capabilities');
  }
}
abstract class DesktopClient {
  Future<Map<String, dynamic>> call(String method, Map<String, dynamic> params);
  Future<void> close();
  Future<Map<String, dynamic>> hello() async {
    final body = await call('hello', {'major': 1, 'minor': 0, 'required': requiredCapabilities});
    checkHello(body);
    return body;
  }
}

class PipeClient extends DesktopClient {
  final Process process;
  final Map<String, Completer<Map<String, dynamic>>> _pending = {};
  final _header = <int>[];
  BytesBuilder _body = BytesBuilder(copy: false);
  int? _length;
  int _serial = 0;
  int stderrBytes = 0;
  bool _closed = false;
  Future<void> _writes = Future<void>.value();
  late final StreamSubscription<List<int>> _stdout;
  late final StreamSubscription<List<int>> _stderr;
  PipeClient._(this.process) {
    _stdout = process.stdout.listen(_data, onError: _fail, onDone: () => _fail(StateError('disconnected')));
    _stderr = process.stderr.listen((chunk) {
      stderrBytes += chunk.length;
      if (stderrBytes > 65536) _fail(StateError('stderr quota'));
    }, onError: _fail);
  }
  static Future<PipeClient> start(String executable) async {
    if (!File(executable).isAbsolute) throw ArgumentError('absolute peer required');
    // No shell, TCP listener, inherited credentials, discovery execution or downloads.
    return PipeClient._(await Process.start(executable, [],
      workingDirectory: File(executable).parent.path,
      includeParentEnvironment: false, environment: const {}));
  }
  void _fail(Object error) {
    if (_closed) return;
    _closed = true;
    process.kill(ProcessSignal.sigkill);
    for (final c in _pending.values) { if (!c.isCompleted) c.completeError(error); }
    _pending.clear();
    _body.clear(); _header.clear(); _length = null;
  }
  void _data(List<int> chunk) {
    if (_closed) return;
    try {
      var cursor = 0;
      while (cursor < chunk.length) {
        if (_length == null) {
          while (_header.length < 4 && cursor < chunk.length) { _header.add(chunk[cursor++]); }
          if (_header.length < 4) return;
          final length = ByteData.sublistView(Uint8List.fromList(_header)).getUint32(0);
          _header.clear();
          if (length == 0 || length > maxResponse) throw StateError('response frame quota');
          _length = length;
        }
        final remaining = _length! - _body.length;
        final size = remaining < chunk.length - cursor ? remaining : chunk.length - cursor;
        _body.add(chunk.sublist(cursor, cursor + size)); cursor += size;
        if (_body.length == _length) {
          final bytes = _body.takeBytes(); _length = null;
          final envelope = jsonDecode(utf8.decode(bytes));
          final id = envelope is Map ? envelope['id'] : null;
          final c = _pending.remove(id);
          if (id is! String || c == null) throw StateError('unknown/duplicate response');
          try { c.complete(decodeResponse(bytes, id)); } catch (e, s) { c.completeError(e, s); }
        }
      }
    } catch (e) { _fail(e); }
  }
  @override
  Future<Map<String, dynamic>> call(String method, Map<String, dynamic> params) async {
    if (_closed) throw StateError('disconnected');
    if (_pending.length >= 16) throw StateError('pending quota');
    final id = 'dart.${++_serial}';
    final bytes = utf8.encode(jsonEncode(request(id, method, params)));
    if (bytes.length > maxRequest) throw StateError('request quota');
    final header = ByteData(4)..setUint32(0, bytes.length);
    final c = Completer<Map<String, dynamic>>();
    _pending[id] = c;
    _writes = _writes.then((_) async {
      if (_closed) return;
      process.stdin.add(header.buffer.asUint8List()); process.stdin.add(bytes);
      await process.stdin.flush();
    }).catchError((Object e) { _fail(e); });
    return c.future.timeout(const Duration(seconds: 5), onTimeout: () {
      final error = TimeoutException('engine deadline'); _fail(error); throw error;
    });
  }
  Future<Map<String, dynamic>> cancelActive(String requestId) => call('cancel', {'request_id': requestId});
  String get lastRequestId => 'dart.$_serial';
  @override
  Future<void> close() async {
    _fail(StateError('closed'));
    await process.exitCode.timeout(const Duration(seconds: 2));
    await _stdout.cancel(); await _stderr.cancel();
    try { await process.stdin.close(); } catch (_) {}
  }
}

typedef _SizeNative = UintPtr Function(UintPtr);
typedef _Size = int Function(int);
typedef _CountNative = UintPtr Function();
typedef _Count = int Function();
typedef _TaskNative = Uint64 Function();
typedef _Task = int Function();
typedef _TaskOpNative = Uint32 Function(Uint64);
typedef _TaskOp = int Function(int);
typedef _FreeNative = Uint32 Function(UintPtr);
typedef _Free = int Function(int);
typedef _CallNative = UintPtr Function(UintPtr, UintPtr, Uint64);
typedef _Call = int Function(int, int, int);

class NativeBindings {
  final DynamicLibrary library;
  late final _Size allocate = library.lookupFunction<_SizeNative, _Size>('fqe_buffer_new');
  late final _Size length = library.lookupFunction<_SizeNative, _Size>('fqe_buffer_len');
  late final _Free free = library.lookupFunction<_FreeNative, _Free>('fqe_buffer_free');
  late final _Count buffers = library.lookupFunction<_CountNative, _Count>('fqe_buffer_count');
  late final _Task newTask = library.lookupFunction<_TaskNative, _Task>('fqe_task_new');
  late final _TaskOp cancel = library.lookupFunction<_TaskOpNative, _TaskOp>('fqe_task_cancel');
  late final _TaskOp freeTask = library.lookupFunction<_TaskOpNative, _TaskOp>('fqe_task_free');
  late final _Count tasks = library.lookupFunction<_CountNative, _Count>('fqe_task_count');
  late final _Call invoke = library.lookupFunction<_CallNative, _Call>('fqe_call');
  NativeBindings(String path) : library = DynamicLibrary.open(path) {
    final abi = library.lookupFunction<Uint32 Function(), int Function()>('fqe_bridge_abi')();
    if (abi != 1) throw StateError('incompatible ABI');
  }
  List<int> call(List<int> bytes, int task) {
    if (bytes.isEmpty || bytes.length > maxRequest) throw StateError('request quota');
    final address = allocate(bytes.length);
    if (address == 0) throw StateError('allocation quota');
    var output = 0;
    try {
      Pointer<Uint8>.fromAddress(address).asTypedList(bytes.length).setAll(0, bytes);
      output = invoke(address, bytes.length, task);
      if (output == 0) throw StateError('native call failed');
      final size = length(output);
      if (size == 0 || size > maxResponse) throw StateError('native output quota');
      return List<int>.from(Pointer<Uint8>.fromAddress(output).asTypedList(size));
    } finally {
      if (output != 0) free(output);
      free(address);
    }
  }
}
List<int> _nativeWorker((String, List<int>, int) args) => NativeBindings(args.$1).call(args.$2, args.$3);
Future<List<int>> _runNative(String path, List<int> bytes, int task) =>
    Isolate.run(() => _nativeWorker((path, bytes, task)));

class FfiClient extends DesktopClient {
  final String path;
  final NativeBindings bindings;
  final Set<int> _active = {};
  final Set<Future<void>> _inflight = {};
  int _serial = 0;
  int? lastTask;
  bool _closed = false;
  FfiClient(this.path) : bindings = NativeBindings(path);
  @override
  Future<Map<String, dynamic>> call(String method, Map<String, dynamic> params) async {
    if (_closed) throw StateError('closed');
    if (_active.length >= 8) throw StateError('task quota');
    final id = 'dart.${++_serial}';
    final bytes = utf8.encode(jsonEncode(request(id, method, params)));
    if (bytes.length > maxRequest) throw StateError('request quota');
    final task = bindings.newTask();
    if (task == 0) throw StateError('task allocation');
    lastTask = task; _active.add(task);
    final done = Completer<void>(); _inflight.add(done.future);
    try {
      final output = await _runNative(path, bytes, task);
      return decodeResponse(output, id);
    } finally {
      bindings.freeTask(task); _active.remove(task);
      done.complete(); _inflight.remove(done.future);
    }
  }
  bool cancelLast() => lastTask != null && bindings.cancel(lastTask!) == 1;
  @override
  Future<void> close() async {
    _closed = true;
    for (final task in _active) { bindings.cancel(task); }
    // No safe forced native interruption. An uncooperative FFI call can block here.
    await Future.wait(_inflight.toList());
    if (bindings.buffers() != 0 || bindings.tasks() != 0) throw StateError('native resource leak');
  }
}
