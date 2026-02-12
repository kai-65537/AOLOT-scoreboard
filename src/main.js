const { listen } = window.__TAURI__.event;

const root = document.querySelector("#scoreboard-root");
const errorBanner = document.querySelector("#error-banner");

function renderSnapshot(snapshot) {
  root.innerHTML = "";
  root.style.backgroundColor = snapshot?.background_color ?? "#000000";

  const components = snapshot?.components ?? [];
  for (const item of components) {
    const node =
      item.component_type === "image"
        ? document.createElement("img")
        : document.createElement("div");

    node.className = `score-item score-item-${item.component_type}`;
    node.dataset.componentId = item.id;
    node.style.left = `${item.x}px`;
    node.style.top = `${item.y}px`;

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
  await listen("scoreboard://state-updated", (event) => {
    hideError();
    renderSnapshot(event.payload);
  });

  await listen("scoreboard://error", (event) => {
    showError(String(event.payload));
  });
});
