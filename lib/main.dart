import 'dart:async';

import 'package:flutter/material.dart';
import 'package:roxy/src/rust/api/error.dart';
import 'package:roxy/src/rust/api/http/proxy.dart';
import 'package:roxy/src/rust/api/utils/logger.dart';
import 'package:roxy/src/rust/frb_generated.dart';

Future<void> main() async {
  await RustLib.init();
  await start();
  runApp(const MyApp());
}

Future<void> start() async {
  setupLogStream(level: LoggingLevel.trace).listen((entry) async {
    print("RUST: ${entry.timeMillis} ${entry.tag} ${entry.msg}");
  }).onError((e) => print("Error setting up log stream: $e"));

  final proxy = ProxyServer(ip: '127.0.0.1', port: 8080);
  proxy.startServer().listen((req) async {
    try {
      var resp = await req.forwardRequest();
      resp.body().listen((chunk) {
        Zone.current.print(chunk);
      }, onError: (e) {
        if (e is BackendErrorImpl) {
          print("ERROR: ${e.display()}");
        }
      });
    } on BackendErrorImpl catch (e) {
      print("ERROR: ${e.display()}");
    }
  });
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(title: const Text('flutter_rust_bridge quickstart')),
        body: const Center(
          child: Text('Action: Call Rust `greet("Tom")`\nResult: 5'),
        ),
      ),
    );
  }
}
