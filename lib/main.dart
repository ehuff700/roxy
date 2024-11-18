import 'package:flutter/material.dart';
import 'package:roxy/src/rust/api/http/proxy.dart';
import 'package:roxy/src/rust/api/http/request.dart';
import 'package:roxy/src/rust/frb_generated.dart';

Future<void> main() async {
  await RustLib.init();
  start();
  runApp(const MyApp());
}

void start() {
  final proxy = ProxyServer(ip: '127.0.0.1', port: 8080);
  proxy.listen().listen((req) async {
    var resp = await req.forwardRequest();
    var body = await resp.method();

    print('${req.toString()}');
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
