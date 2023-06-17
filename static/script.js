"use strict";

const form = document.querySelector("#passphrase-form");
form.addEventListener("submit", async (event) => {
  event.preventDefault();
  // Replace the form with the video container
  document.querySelector(".container").style.display = "none";
  document.querySelector("#video-container").style.display = "flex";
});

async function onBtnClick(event) {
  var btn = event.target;
  var status = btn.getAttribute("data-status");
  if (status === "active") {
    btn.textContent = btn.textContent.replace("Mute", "UnMute");
    btn.setAttribute("data-status", "muted");
  } else if (status === "muted") {
    btn.textContent = btn.textContent.replace("UnMute", "Mute");
    btn.setAttribute("data-status", "active");
  }
}

var btns = document.getElementsByClassName("btn");
for (var i = 0; i < btns.length; i++) {
  btns[i].addEventListener("click", onBtnClick);
}
