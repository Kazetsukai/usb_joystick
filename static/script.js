function checkState() {
  fetch("./state")
    .then((response) => response.json())
    .then((data) => {
      document.querySelector("#boolToggle").checked = data;
    });
}

function debounce_leading(func, timeout = 300) {
  let timer;
  return (...args) => {
    if (!timer) {
      func(...args);
    }
    clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
    }, timeout);

    return false;
  };
}

document.addEventListener("DOMContentLoaded", function () {
  document.querySelector("#boolToggle").addEventListener(
    "click",
    debounce_leading(function () {
      fetch("./toggle", { method: "POST" });
      return false;
    })
  );

  let params = new URLSearchParams(window.location.search);
  if (params.has("watch")) {
    setInterval(checkState, 1000);
  }
});
