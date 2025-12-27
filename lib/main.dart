import 'package:flutter/material.dart';
import 'package:my_app/src/rust/api/simple.dart';
import 'package:my_app/src/rust/frb_generated.dart';
import 'package:path_provider/path_provider.dart';
import 'package:my_app/src/save/save_service.dart' as saves;

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  final docsDir = await getApplicationDocumentsDirectory();
  setApplicationDocumentsDirectory(dir: docsDir.path);
  final supportDir = await getApplicationSupportDirectory();
  initSystem(basePath: supportDir.path);
  final debugDir =
      debugApplicationDocumentsDirectory() ?? 'Directory unavailable';
  final saveService = saves.SaveService();
  runApp(MyApp(documentsDir: debugDir, saveService: saveService));
}

class MyApp extends StatelessWidget {
  const MyApp({
    super.key,
    required this.documentsDir,
    required this.saveService,
  });

  final String documentsDir;
  final saves.SaveService saveService;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: SaveManagementPage(
        service: saveService,
        documentsDir: documentsDir,
      ),
    );
  }
}

class SaveManagementPage extends StatefulWidget {
  const SaveManagementPage({
    super.key,
    required this.service,
    required this.documentsDir,
  });

  final saves.SaveService service;
  final String documentsDir;

  @override
  State<SaveManagementPage> createState() => _SaveManagementPageState();
}

class _SaveManagementPageState extends State<SaveManagementPage> {
  List<saves.SaveSlotMetadata> _slots = const [];
  bool _loading = true;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _refreshSlots();
  }

  @override
  void dispose() {
    widget.service.dispose();
    super.dispose();
  }

  Future<void> _refreshSlots() async {
    setState(() {
      _loading = true;
      _errorMessage = null;
    });
    try {
      final slots = widget.service.fetchSlots();
      setState(() {
        _slots = slots;
        _loading = false;
      });
    } catch (err) {
      setState(() {
        _loading = false;
        _errorMessage = err.toString();
      });
    }
  }

  Future<void> _createSave() async {
    try {
      final timestamp = DateTime.now().toLocal().toIso8601String();
      final slot = widget.service.createSlot('Save $timestamp');
      _showSnack('Created ${slot.name}');
      await _refreshSlots();
    } catch (err) {
      _showSnack('Failed to create save: $err');
    }
  }

  Future<void> _deleteSlot(saves.SaveSlotMetadata slot) async {
    try {
      widget.service.removeSlot(slot.id);
      _showSnack('Deleted ${slot.name}');
      await _refreshSlots();
    } catch (err) {
      _showSnack('Failed to delete save: $err');
    }
  }

  void _showSnack(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_errorMessage != null) {
      return Center(child: Text(_errorMessage!));
    }
    if (_slots.isEmpty) {
      return const Center(
        child: Text('No saves yet. Tap below to create one.'),
      );
    }

    return RefreshIndicator(
      onRefresh: _refreshSlots,
      child: ListView.separated(
        padding: const EdgeInsets.all(16),
        itemCount: _slots.length,
        separatorBuilder: (_, __) => const SizedBox(height: 12),
        itemBuilder: (context, index) {
          final slot = _slots[index];
          return Card(
            child: ListTile(
              title: Text(slot.name),
              subtitle: Text(
                'Last played: ${slot.lastPlayed}\nPath: ${slot.filePath}',
              ),
              isThreeLine: true,
              trailing: IconButton(
                onPressed: () => _deleteSlot(slot),
                icon: const Icon(Icons.delete),
                tooltip: 'Delete save',
              ),
            ),
          );
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Save Slots')),
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Text(
              'Application documents directory:\n${widget.documentsDir}',
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: ElevatedButton.icon(
              onPressed: _loading ? null : _createSave,
              icon: const Icon(Icons.add),
              label: const Text('Create New Save'),
            ),
          ),
          const SizedBox(height: 8),
          Expanded(child: _buildBody()),
        ],
      ),
    );
  }
}
