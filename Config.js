.pragma library

// Defaults for ~/.config/omapaste/qml-config.json (optional overrides via FileView).

var DEFAULTS = {
  default_keep: "1d",
  max_items: 300,
  max_bytes: 8000000,
  paste_keys: "auto",
  ignore_secrets: true
}

function normalize(raw) {
  var cfg = {
    default_keep: DEFAULTS.default_keep,
    max_items: DEFAULTS.max_items,
    max_bytes: DEFAULTS.max_bytes,
    paste_keys: DEFAULTS.paste_keys,
    ignore_secrets: DEFAULTS.ignore_secrets
  }
  if (!raw || typeof raw !== "object")
    return cfg
  if (raw.default_keep === "1h" || raw.default_keep === "1d" || raw.default_keep === "7d" || raw.default_keep === "forever")
    cfg.default_keep = raw.default_keep
  var max = Number(raw.max_items)
  if (!isNaN(max) && max >= 1)
    cfg.max_items = Math.floor(max)
  var maxBytes = Number(raw.max_bytes)
  if (!isNaN(maxBytes) && maxBytes >= 1)
    cfg.max_bytes = Math.floor(maxBytes)
  if (raw.paste_keys === "auto" || raw.paste_keys === "shift-insert" || raw.paste_keys === "ctrl-v")
    cfg.paste_keys = raw.paste_keys
  if (typeof raw.ignore_secrets === "boolean")
    cfg.ignore_secrets = raw.ignore_secrets
  return cfg
}

function parse(text) {
  try {
    return normalize(JSON.parse(String(text || "{}")))
  } catch (e) {
    return normalize(null)
  }
}
