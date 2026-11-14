// Run with Desk's pinned Dart VM; no package resolution or Flutter dependency.
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'client.dart';

void require(bool condition, String detail) { if (!condition) throw StateError(detail); }
Map<String, dynamic> fixture(String root) {
  final base = '$root/crates/fqe-runtime/tests/fixtures/p3';
  return {
    'kind': 'event', 'model': File('$base/backtest-events.json').readAsStringSync(),
    'parameters': File('$root/crates/fqe-cli/tests/fixtures/parameters-60.json').readAsStringSync(),
    'history_spec': File('$base/backtest-history-spec.json').readAsStringSync(),
    'csv': File('$base/backtest-history.csv').readAsStringSync(),
    'start_ns': '1704153600000000000', 'end_ns': '1704154020000000000',
    'warmup_bars': '0', 'run': 'desktop.candidate', 'seed': '18446744073709551615',
    'account': {'id': 'local.research', 'profile': 'cash_long', 'initial_cash': '100.00',
      'participation_bps': '10000', 'fee_bps': '0', 'minimum_fee': '0.50',
      'slippage_bps': '0', 'risk': null},
    'policies': [
      {'id': 'entry', 'source': {'kind': 'long_signal'},
        'action': {'kind': 'buy_if_flat', 'quantity': '2.000', 'cash_cap': '20.00'}},
      {'id': 'exit', 'source': {'kind': 'event_boolean', 'name': 'exit'},
        'action': {'kind': 'close_long'}},
    ],
  };
}
Future<bool> rejected(Future<Object?> operation) async {
  try { await operation; return false; } catch (_) { return true; }
}
Future<Map<String, dynamic>> measure(DesktopClient client, Map<String, dynamic> data,
    Stopwatch cold, String output) async {
  final hello = await client.hello(); cold.stop();
  final samples = <int>[];
  for (var i = 0; i < 100; i++) {
    final watch = Stopwatch()..start();
    final echo = await client.call('echo', {'u64': '18446744073709551615',
      'ns': '1704153600000000001', 'decimal': '0.000000000001', 'present': null});
    samples.add(watch.elapsedMicroseconds);
    require(echo['u64'] == '18446744073709551615' && echo.containsKey('present') &&
      echo['present'] == null && echo['decimal'] == '0.000000000001', 'exact/presence');
  }
  samples.sort(); final p95 = samples[94] / 1000;
  require(p95 <= 250, 'warm budget');
  require(await rejected(client.call('hello', {'major': 2, 'minor': 0, 'required': []})), 'major rejection');
  require(await rejected(client.call('hello', {'major': 1, 'minor': 0, 'required': ['unknown']})), 'capability rejection');
  require(await rejected(client.call('echo', {'large': 'x' * maxRequest})), 'input bound');
  final model = {'kind': data['kind'], 'model': data['model'], 'parameters': data['parameters']};
  final validation = await client.call('model.validate', model);
  final descriptors = await client.call('descriptor.list', model);
  require((descriptors['descriptors'] as List).isNotEmpty &&
    validation['registry_id'] == descriptors['registry_id'], 'descriptor discovery');
  final clock = Stopwatch()..start(); var previous = 0; var maxGapUs = 0; var ticks = 0;
  final timer = Timer.periodic(const Duration(milliseconds: 1), (_) {
    final now = clock.elapsedMicroseconds;
    final gap = now - previous; previous = now;
    if (gap > maxGapUs) maxGapUs = gap; ticks++;
  });
  final report = await client.call('backtest.run', data);
  await Future<void>.delayed(const Duration(milliseconds: 5)); timer.cancel();
  final financialUs = clock.elapsedMicroseconds;
  final value = report['report'] as Map<String, dynamic>;
  require(value['terminal'] == 'completed' && value['metrics']['final_equity'] == '97.00' &&
    value['metrics']['net_profit'] == '-3.00' && value['metrics']['fees'] == '1.00' &&
    value['manifest']['seed'] == '18446744073709551615', 'actual financial/exact seed');
  require(ticks > 0 && maxGapUs <= 250000, 'host responsiveness');
  File(output).writeAsStringSync(jsonEncode(value));
  final cancelWatch = Stopwatch()..start();
  final future = client.call('backtest.run', data);
  bool cancelAccepted;
  if (client is FfiClient) {
    cancelAccepted = client.cancelLast();
  } else {
    final pipe = client as PipeClient;
    final id = pipe.lastRequestId;
    cancelAccepted = (await pipe.cancelActive(id))['accepted'] == true;
  }
  final cancelled = await future;
  final terminal = cancelled['report']['terminal'];
  require(cancelAccepted && terminal == 'cancelled', 'bar-safe cancellation acknowledged');
  require(cancelWatch.elapsedMilliseconds <= 2000, 'cancel budget');
  final result = {
    'cold_ms': cold.elapsedMicroseconds / 1000, 'echo_p95_ms': p95,
    'cold_budget_pass': cold.elapsedMilliseconds <= 3000,
    'financial_ms': financialUs / 1000,
    'host_timer_max_gap_ms': maxGapUs / 1000, 'host_timer_ticks': ticks,
    'cancel_ms': cancelWatch.elapsedMicroseconds / 1000, 'cancel_terminal': terminal,
    'bridge_id': hello['bridge_id'], 'result_id': report['result_id'],
    'descriptors': (descriptors['descriptors'] as List).length,
  };
  await client.close();
  if (client is FfiClient) {
    result['remaining_buffers'] = client.bindings.buffers();
    result['remaining_tasks'] = client.bindings.tasks();
  } else { result['peer_exit'] = await (client as PipeClient).process.exitCode; }
  return result;
}
Future<void> main(List<String> args) async {
  if (args.length != 3) throw ArgumentError('ROOT ARTIFACT_DIRECTORY OUTPUT_DIRECTORY');
  final root = Directory(args[0]).absolute.path;
  final artifacts = Directory(args[1]).absolute.path;
  final out = Directory(args[2])..createSync(recursive: true);
  final data = fixture(root);
  File('${out.path}/request.json').writeAsStringSync(jsonEncode(request('bundle.1', 'backtest.run', data)));
  final ffiWatch = Stopwatch()..start();
  final ffi = FfiClient('$artifacts/libfqe_bridge.dylib');
  final ffiResult = await measure(ffi, data, ffiWatch, '${out.path}/ffi-report.json');
  final ipcWatch = Stopwatch()..start();
  final ipc = await PipeClient.start('$artifacts/fqe-desktop-peer');
  final ipcResult = await measure(ipc, data, ipcWatch, '${out.path}/ipc-report.json');
  require(File('${out.path}/ffi-report.json').readAsStringSync() ==
    File('${out.path}/ipc-report.json').readAsStringSync(), 'FFI/IPC byte parity');
  final death = await PipeClient.start('$artifacts/fqe-desktop-peer');
  await death.hello(); final deathWatch = Stopwatch()..start();
  final pending = death.call('backtest.run', data);
  death.process.kill(ProcessSignal.sigkill);
  require(await rejected(pending), 'peer death pending failure');
  await death.close();
  require(deathWatch.elapsedMilliseconds <= 2000, 'death cleanup budget');
  final report = {'format': 'fqe.desktop-candidate-measurement/1',
    'timestamp_utc': DateTime.now().toUtc().toIso8601String(),
    'dart': Platform.version, 'os': Platform.operatingSystem, 'ffi': ffiResult, 'ipc': ipcResult,
    'peer_death_cleanup_ms': deathWatch.elapsedMicroseconds / 1000,
    'full_report_byte_parity': true, 'packages': 'SDK standard libraries only'};
  File('${out.path}/comparison.json').writeAsStringSync(const JsonEncoder.withIndent('  ').convert(report));
  stdout.writeln(jsonEncode(report));
  // Rejected candidate evidence is retained. The selected IPC candidate must meet the budget.
  require(ipcResult['cold_budget_pass'] == true, 'selected IPC cold handshake budget (measurements saved)');
}
