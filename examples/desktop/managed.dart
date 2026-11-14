import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'managed_client.dart';
import 'compare.dart' show fixture, require, rejected;

Future<void> main(List<String> args) async {
  if (args.length != 3) throw ArgumentError('ROOT ARTIFACT_DIRECTORY OUTPUT_DIRECTORY');
  final root = Directory(args[0]).absolute.path;
  final artifacts = Directory(args[1]).absolute.path;
  final out = Directory(args[2])..createSync(recursive: true);
  final client = await ManagedClient.start('$artifacts/fqe-desktop-peer');
  final seen = <String>[];
  final firstWindow = client.updates.listen((v) { seen.add('${v.handle}:${v.cursor}:${v.phase}'); });
  final frozenDraft = fixture(root);
  final starting = client.startRun('first.operation', frozenDraft);
  frozenDraft['seed'] = '123';
  final first = await starting;
  await firstWindow.cancel(); // Closing this view does not close the application-owned service.
  final otherWindow = client.updates.listen((v) { seen.add('${v.handle}:${v.cursor}:${v.phase}'); });
  final terminal = await client.wait(first.handle);
  require(terminal.phase == 'completed' && terminal.processedBars == '7', 'managed native terminal');
  final bytes = await client.resultBytes(first.handle);
  File('${out.path}/managed-report.json').writeAsBytesSync(bytes);
  final report = jsonDecode(utf8.decode(bytes));
  require(report['metrics']['final_equity'] == '97.00' &&
    report['manifest']['seed'] == '18446744073709551615', 'frozen native values');
  final again = await client.startRun('first.operation', fixture(root));
  require(again.handle == first.handle, 'idempotent native operation');
  require(await rejected(client.startRun('first.operation', frozenDraft)), 'same operation changed input rejected');
  final page = await client.output(first.handle, 'fills', 0, limit: 1);
  require(page['rows'][0]['source_bar'] == '6' && page['next_cursor'] == '1' && page['eof'] == false, 'native debug cursor');
  final watch = Stopwatch()..start();
  final stopping = await client.startRun('stop.operation', fixture(root));
  await client.stop(stopping.handle);
  final stopped = await client.wait(stopping.handle);
  require(['cancelled', 'completed'].contains(stopped.phase), 'stop-complete race explicit');
  require(watch.elapsedMilliseconds <= 2000, 'stop terminal deadline');
  final stopMs = watch.elapsedMicroseconds / 1000;
  await client.release(stopping.handle);
  final bad = await client.startRun('invalid.operation', {});
  final badTerminal = await client.wait(bad.handle);
  require(badTerminal.phase == 'failed' && badTerminal.errorCode != null &&
    badTerminal.resultId == null, 'failure never retains previous result');
  await client.release(bad.handle);
  final controls = <int>[];
  for (var i = 0; i < 100; i++) {
    final clock = Stopwatch()..start(); await client.refresh(first.handle);
    controls.add(clock.elapsedMicroseconds);
  }
  controls.sort(); final controlP95 = controls[94] / 1000;
  require(controlP95 <= 250, 'control p95 budget');
  await client.release(first.handle); await client.release(first.handle);
  final released = await client.startRun('first.operation', fixture(root));
  require(released.phase == 'released', 'tombstone prevents implicit rerun');
  await otherWindow.cancel(); await client.close();
  require(await client.pipe.process.exitCode == 0, 'graceful peer EOF cleanup');
  // A failed process invalidates active views; no automatic retry or backend switch.
  final lost = await ManagedClient.start('$artifacts/fqe-desktop-peer');
  final losing = await lost.startRun('crash.operation', fixture(root));
  lost.pipe.process.kill(ProcessSignal.sigkill);
  require(await rejected(lost.refresh(losing.handle)), 'peer failure visible');
  require(lost.runs[losing.handle]!.phase == 'interrupted', 'stale success cleared');
  await lost.close();
  final result = {'format': 'fqe.managed-desktop-journey/1', 'status': 'passed',
    'dart': Platform.version, 'timestamp_utc': DateTime.now().toUtc().toIso8601String(),
    'session': client.session, 'final_equity': report['metrics']['final_equity'],
    'result_id': terminal.resultId, 'stop_ms': stopMs, 'stop_terminal': stopped.phase,
    'control_p95_ms': controlP95, 'observed_views': seen,
    'window_close_preserved_run': true, 'exact_chunk_reassembly': true,
    'same_operation_no_rerun': true, 'release_tombstone': true,
    'failure_and_disconnect_clear_success': true, 'graceful_peer_exit': 0};
  File('${out.path}/journey.json').writeAsStringSync(const JsonEncoder.withIndent('  ').convert(result));
  stdout.writeln(jsonEncode(result));
}
