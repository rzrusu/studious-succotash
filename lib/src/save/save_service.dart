import 'dart:async';

import '../rust/api/simple.dart' as bridge;

typedef SaveSlotMetadata = bridge.SaveSlotMetadata;
typedef PlayerData = bridge.PlayerData;

/// High-level wrapper around the Rust save APIs exposed via flutter_rust_bridge.
class SaveService {
  SaveService();

  static const Duration _debounceInterval = Duration(seconds: 5);

  String? _activeSlotId;
  DateTime? _lastSaveAt;
  Timer? _pendingSaveTimer;

  String? get activeSlotId => _activeSlotId;

  List<SaveSlotMetadata> fetchSlots() => bridge.getAllSlots();

  SaveSlotMetadata createSlot(String displayName) =>
      bridge.createNewSlot(displayName: displayName);

  void loadSlotById(String slotId) {
    bridge.loadSlot(slotId: slotId);
    _activeSlotId = slotId;
    _pendingSaveTimer?.cancel();
    _pendingSaveTimer = null;
  }

  void saveNow(PlayerData data) {
    bridge.savePlayerData(data: data);
    _lastSaveAt = DateTime.now();
  }

  void removeSlot(String slotId) {
    bridge.deleteSlot(slotId: slotId);
    if (_activeSlotId == slotId) {
      _activeSlotId = null;
      _pendingSaveTimer?.cancel();
      _pendingSaveTimer = null;
    }
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
