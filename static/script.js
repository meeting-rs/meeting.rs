"use strict";

const form = document.querySelector("#passphrase-form");
form.addEventListener("submit", async (event) => {
  event.preventDefault();
  // Replace the form with the video container
  document.querySelector(".container").style.display = "none";
  document.querySelector("#video-container").style.display = "flex";
});
