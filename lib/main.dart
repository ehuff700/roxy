import 'dart:async';

import 'package:flutter/material.dart';
import 'package:roxy/backend/api/http/server.dart';
import 'package:roxy/backend/api/utils/logger.dart';
import 'package:roxy/backend/frb_generated.dart';
import 'package:roxy/utils/logging.dart';

const kMaxLoggingLevel = LoggingLevel.trace;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized(); // Ensure Flutter is initialized
  await RustLib.init(); // Initialize rust library
  DLogger.init(); // Initialize logging
  await setup();
  runApp(const MyApp());
}

Future<void> setup() async {
  final proxy = ProxyServer(config: ProxyConfig.default_());
  proxy.proxyRequest(
    onRequest: (req) async => req,
    onResponse: (resp) async {
      DLogger.d("RESPONSE: ${resp.requestId}");
      return resp;
    },
  );
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(title: const Text('flutter_rust_bridge quickstart')),
        body: const Center(
          child: Column(
            children: [
              Text("blah"),
              Text('blah'),
            ],
          ),
        ),
      ),
    );
  }
}
