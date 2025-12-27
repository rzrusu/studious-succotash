import 'package:flutter/material.dart';
import 'package:my_app/src/rust/api/simple.dart';
import 'package:my_app/src/rust/frb_generated.dart';
import 'package:path_provider/path_provider.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final docsDir = await getApplicationDocumentsDirectory();
  setApplicationDocumentsDirectory(dir: docsDir.path);
  final debugDir = debugApplicationDocumentsDirectory() ?? 'Directory unavailable';
  runApp(MyApp(debugDir: debugDir));
}

class MyApp extends StatelessWidget {
  const MyApp({super.key, required this.debugDir});

  final String debugDir;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: Scaffold(
        appBar: AppBar(title: const Text('flutter_rust_bridge quickstart')),
        body: Center(
          child: Text('Application documents directory:\n$debugDir'),
        ),
      ),
    );
  }
}
