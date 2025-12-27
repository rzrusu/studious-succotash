import 'dart:async';

import '../rust/api/simple.dart';

/// High-level wrapper around the Rust save APIs exposed via flutter_rust_bridge.
class SaveService {
  SaveService();

  static const Duration _debounceInterval = Duration(seconds: 5);

  String? _activeSlotId;
  DateTime? _lastSaveAt;
  Timer? _pendingSaveTimer;

  String? get activeSlotId => _activeSlotId;

  List<SaveSlotMetadata> fetchSlots() => getAllSlots();

  SaveSlotMetadata createSlot(String displayName) =>
      createNewSlot(displayName: displayName);

  void loadSlotById(String slotId) {
    loadSlot(slotId: slotId);
    _activeSlotId = slotId;
    _pendingSaveTimer?.cancel();
    _pendingSaveTimer = null;
  }

  void saveNow(PlayerData data) {
    savePlayerData(data: data);
    _lastSaveAt = DateTime.now();
  }

  /// Throttles saves so Rust is called at most once every five seconds.
  void debouncedSave(PlayerData data) {
    final now = DateTime.now();
    final lastSave = _lastSaveAt;

    if (lastSave == null || now.difference(lastSave) >= _debounceInterval) {
      _pendingSaveTimer?.cancel();
      saveNow(data);
      return;
    }

    final waitDuration = _debounceInterval - now.difference(lastSave);
    _pendingSaveTimer?.cancel();
    _pendingSaveTimer = Timer(waitDuration, () {
      _pendingSaveTimer = null;
      saveNow(data);
    });
  }

  void dispose() {
    _pendingSaveTimer?.cancel();
    _pendingSaveTimer = null;
  }
}
