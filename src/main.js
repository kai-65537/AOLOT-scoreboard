const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;

const root = document.querySelector("#scoreboard-root");
const errorBanner = document.querySelector("#error-banner");
const hotkeyToggle = document.querySelector("#hotkey-toggle");
const hotkeyToggleHotspot = document.querySelector("#hotkey-toggle-hotspot");
const editDialog = document.querySelector("#label-edit-dialog");
const editForm = document.querySelector("#label-edit-form");
const editInput = document.querySelector("#label-edit-input");
const editTitle = document.querySelector("#label-edit-title");
const editCancel = document.querySelector("#label-edit-cancel");

let editingLabelId = null;
let manualHotkeysPaused = false;
let appliedHotkeysPaused = false;
let isWindowActive = document.hasFocus();
let isHotkeyToggleHotspotHovered = false;
let isHotkeyToggleHovered = false;

async function setHotkeysPaused(paused) {
  await invoke("set_hotkeys_paused", { paused });
}

function shouldPauseHotkeys() {
  return manualHotkeysPaused || editingLabelId !== null;
}

function updateHotkeyToggleUi() {
  const showToggle =
    isWindowActive &&
    (manualHotkeysPaused || isHotkeyToggleHotspotHovered || isHotkeyToggleHovered);
  hotkeyToggle.hidden = !showToggle;
  hotkeyToggle.dataset.paused = manualHotkeysPaused ? "true" : "false";
  hotkeyToggle.setAttribute("aria-pressed", manualHotkeysPaused ? "true" : "false");
  hotkeyToggle.textContent = manualHotkeysPaused ? "Resume Key Capture" : "Pause Key Capture";
}

async function syncHotkeyPauseState() {
  const shouldPause = shouldPauseHotkeys();
  if (shouldPause === appliedHotkeysPaused) {
    return;
  }

  await setHotkeysPaused(shouldPause);
  appliedHotkeysPaused = shouldPause;
}

async function openLabelEditor(item) {
  const previousEditingLabelId = editingLabelId;
  editingLabelId = item.id;

  try {
    await syncHotkeyPauseState();
    hideError();
  } catch (error) {
    editingLabelId = previousEditingLabelId;
    await syncHotkeyPauseState().catch(() => {});
    showError(String(error));
    return;
  }

  editTitle.textContent = `Edit ${item.id}`;
  editInput.value = item.text ?? "";
  if (!editDialog.open) {
    editDialog.showModal();
  }
  editInput.focus();
  editInput.select();
}

function renderSnapshot(snapshot) {
  root.innerHTML = "";
  root.style.backgroundColor = snapshot?.background_color ?? "#000000";

  const components = snapshot?.components ?? [];
  for (const item of [...components].reverse()) {
    const node =
      item.component_type === "image"
        ? document.createElement("img")
        : document.createElement("div");

    node.className = `score-item score-item-${item.component_type}`;
    node.dataset.componentId = item.id;
    node.style.left = `${item.x}px`;
    node.style.top = `${item.y}px`;
    const centered =
      item.alignment === "center" &&
      (item.component_type === "number" ||
        item.component_type === "timer" ||
        item.component_type === "label");
    node.style.transform = centered ? "translate(-50%, -50%)" : "";

    if (item.component_type === "image") {
      if (item.width) node.style.width = `${item.width}px`;
      if (item.height) node.style.height = `${item.height}px`;
      if (item.opacity != null) node.style.opacity = String(item.opacity);

      const srcValue = item.source ?? "";
      const convertFileSrc = window.__TAURI__.core?.convertFileSrc;
      node.src = typeof convertFileSrc === "function" ? convertFileSrc(srcValue) : srcValue;
      node.alt = item.id;
    } else {
      node.style.fontFamily = item.font_family;
      node.style.fontSize = `${item.font_size}px`;
      node.style.color = item.font_color;
      node.textContent = item.text ?? "";

      if (item.component_type === "label" && item.editable) {
        node.style.cursor = "pointer";
        node.title = `Click to edit ${item.id}`;
        node.addEventListener("click", () => {
          void openLabelEditor(item);
        });
      }
    }

    root.appendChild(node);
  }
}

function showError(message) {
  errorBanner.textContent = message;
  errorBanner.hidden = false;
}

function hideError() {
  errorBanner.hidden = true;
  errorBanner.textContent = "";
}

window.addEventListener("DOMContentLoaded", async () => {
  updateHotkeyToggleUi();

  hotkeyToggleHotspot.addEventListener("mouseenter", () => {
    isHotkeyToggleHotspotHovered = true;
    updateHotkeyToggleUi();
  });

  hotkeyToggleHotspot.addEventListener("mouseleave", () => {
    isHotkeyToggleHotspotHovered = false;
    updateHotkeyToggleUi();
  });

  hotkeyToggle.addEventListener("click", async () => {
    const previousManualHotkeysPaused = manualHotkeysPaused;
    manualHotkeysPaused = !manualHotkeysPaused;
    updateHotkeyToggleUi();

    try {
      await syncHotkeyPauseState();
      hideError();
    } catch (error) {
      manualHotkeysPaused = previousManualHotkeysPaused;
      updateHotkeyToggleUi();
      showError(String(error));
    }
  });

  hotkeyToggle.addEventListener("mouseenter", () => {
    isHotkeyToggleHovered = true;
    updateHotkeyToggleUi();
  });

  hotkeyToggle.addEventListener("mouseleave", () => {
    isHotkeyToggleHovered = false;
    updateHotkeyToggleUi();
  });

  window.addEventListener("focus", () => {
    isWindowActive = true;
    updateHotkeyToggleUi();
  });

  window.addEventListener("blur", () => {
    isWindowActive = false;
    isHotkeyToggleHotspotHovered = false;
    isHotkeyToggleHovered = false;
    updateHotkeyToggleUi();
  });

  editCancel.addEventListener("click", () => {
    editDialog.close();
  });

  editDialog.addEventListener("close", async () => {
    editingLabelId = null;
    try {
      await syncHotkeyPauseState();
      hideError();
    } catch (error) {
      showError(String(error));
    }
  });

  editForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!editingLabelId) {
      editDialog.close();
      return;
    }

    try {
      await invoke("update_label_text", {
        id: editingLabelId,
        value: editInput.value,
      });
      editingLabelId = null;
      editDialog.close();
      hideError();
    } catch (error) {
      showError(String(error));
    }
  });

  await listen("scoreboard://state-updated", (event) => {
    hideError();
    renderSnapshot(event.payload);
  });

  await listen("scoreboard://error", (event) => {
    showError(String(event.payload));
  });
});
