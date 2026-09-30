/* Track2Mix site — email capture and tip-jar wiring.
 *
 * Two constants need filling in before launch. Both are documented in
 * site/README.md. Neither fails silently: if a constant is empty, the affected
 * control tells the visitor it isn't wired up and logs the reason, rather than
 * pretending to work.
 */

/* ------------------------------------------------------------------ config */

/** List-provider form endpoint. Any provider accepting a form-encoded POST. */
const SIGNUP_ENDPOINT = "";

/** Field name the provider expects for the address. Buttondown uses "email". */
const EMAIL_FIELD = "email";

/**
 * Tip-jar base URL. Ko-fi and Buy Me a Coffee both accept a suggested amount
 * as a query parameter, so the tiers deep-link straight to a prefilled amount.
 *   Ko-fi:  https://ko-fi.com/<handle>
 *   Stripe: a Payment Link that permits a customer-chosen amount
 */
const TIP_BASE_URL = "";

/** Built for TIP_BASE_URL; adjust if your provider names the param differently. */
function tipUrlFor(amount) {
  const url = new URL(TIP_BASE_URL);
  url.searchParams.set("amount", amount);
  return url.toString();
}

/* ------------------------------------------------------------------- utils */

function setMsg(el, state, text) {
  el.dataset.state = state;
  el.textContent = text;
}

/* ------------------------------------------------------- email signup form */

const form = document.getElementById("signup-form");
const input = document.getElementById("email");
const submit = document.getElementById("signup-submit");
const formMsg = document.getElementById("form-msg");

/** Deliberately permissive: reject only what is obviously not an address. */
function looksLikeEmail(value) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(value);
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();

  const email = input.value.trim();
  if (!looksLikeEmail(email)) {
    setMsg(formMsg, "error", "That doesn't look like an email address — check it and try again.");
    input.focus();
    return;
  }

  if (!SIGNUP_ENDPOINT) {
    setMsg(
      formMsg,
      "error",
      "Signup isn't wired up yet — no form endpoint is configured. " +
        "Grab the build from GitHub in the meantime."
    );
    console.error(
      "Track2Mix site: SIGNUP_ENDPOINT is empty in main.js. " +
        "Set it to your list provider's form endpoint (see site/README.md)."
    );
    return;
  }

  submit.disabled = true;
  const originalLabel = submit.textContent;
  submit.textContent = "Sending…";
  setMsg(formMsg, "", "");

  try {
    const body = new FormData();
    body.append(EMAIL_FIELD, email);

    const response = await fetch(SIGNUP_ENDPOINT, {
      method: "POST",
      body,
      headers: { Accept: "application/json" },
    });

    if (!response.ok) {
      const detail = (await response.text()).slice(0, 300);
      throw new Error(
        `${response.status} ${response.statusText}${detail ? ` — ${detail}` : ""}`
      );
    }

    form.reset();
    setMsg(formMsg, "ok", "You're on the list. Check your inbox for the download link.");
  } catch (error) {
    // Show the real failure: a vague "something went wrong" costs signups and
    // hides outages.
    setMsg(formMsg, "error", `Signup failed: ${error.message}. Please try again.`);
    console.error("Track2Mix site: signup POST failed.", error);
  } finally {
    submit.disabled = false;
    submit.textContent = originalLabel;
  }
});

/* ------------------------------------------------------------- tip tiers */

const tipMsg = document.getElementById("tip-msg");
const tiers = Array.from(document.querySelectorAll("#tip-tiers .tier"));

if (!TIP_BASE_URL) {
  // Disable rather than link nowhere. The console line names the exact fix.
  for (const tier of tiers) {
    tier.setAttribute("aria-disabled", "true");
    tier.removeAttribute("href");
  }
  setMsg(
    tipMsg,
    "error",
    "The tip jar isn't connected yet — the app is still free to download above."
  );
  console.error(
    "Track2Mix site: TIP_BASE_URL is empty in main.js. " +
      "Set it to your Ko-fi, Buy Me a Coffee or Stripe Payment Link URL (see site/README.md)."
  );
} else {
  for (const tier of tiers) {
    const amount = tier.dataset.amount;
    if (!amount) {
      console.error("Track2Mix site: a .tier element is missing data-amount.", tier);
      continue;
    }
    tier.href = tipUrlFor(amount);
    tier.target = "_blank";
    tier.rel = "noopener noreferrer";
  }
}
