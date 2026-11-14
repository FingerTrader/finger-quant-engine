// Exercise the actual client against deterministic first-party hostile pipe fixtures.
import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'client.dart';
Future<void> main(List<String> args) async {
  if (args.length != 1) throw ArgumentError('FAULT_FIXTURE_DIRECTORY');
  final cases = <Map<String, dynamic>>[];
  for (final mode in ['oversized', 'stderr', 'partial', 'wrongid', 'json', 'timeout', 'duplicate']) {
    final client = await PipeClient.start('${Directory(args[0]).absolute.path}/$mode');
    final watch = Stopwatch()..start();
    var rejected = false;
    try {
      await client.call('echo', {});
      // A duplicate reply completes the first request once, then disconnects the session.
      await Future<void>.delayed(const Duration(milliseconds: 20));
      await client.call('echo', {});
    } catch (_) { rejected = true; }
    if (!rejected) throw StateError('fixture accepted: $mode');
    final failureMs = watch.elapsedMilliseconds;
    final cleanup = Stopwatch()..start();
    await client.close();
    if (cleanup.elapsedMilliseconds > 2000) throw StateError('cleanup deadline');
    cases.add({'case': mode, 'failed_after_ms': failureMs,
      'cleanup_ms': cleanup.elapsedMicroseconds / 1000, 'exit': await client.process.exitCode,
      'stderr_bytes_counted': client.stderrBytes});
  }
  stdout.writeln(jsonEncode({'format': 'fqe.desktop-client-failures/1', 'status': 'passed', 'cases': cases}));
}
