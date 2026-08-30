.pragma library

// Keep presets matching src/store.rs KEEP_PRESETS.
var KEEP_PRESETS = [
  { key: "1h", label: "1h", seconds: 3600 },
  { key: "1d", label: "1d", seconds: 86400 },
  { key: "7d", label: "7d", seconds: 604800 },
  { key: "forever", label: "∞", seconds: null }
]

var DEFAULT_KEEP = "1d"
var DEFAULT_MAX_ITEMS = 200

function keepByKey(key) {
  var k = String(key || "")
  for (var i = 0; i < KEEP_PRESETS.length; i++) {
    if (KEEP_PRESETS[i].key === k)
      return KEEP_PRESETS[i]
  }
  return null
}

function nextKeep(current) {
  var index = 1
  for (var i = 0; i < KEEP_PRESETS.length; i++) {
    if (KEEP_PRESETS[i].key === String(current || "")) {
      index = i
      break
    }
  }
  return KEEP_PRESETS[(index + 1) % KEEP_PRESETS.length]
}

function keepUntilFrom(preset, nowSecs) {
  var spec = keepByKey(preset) || keepByKey(DEFAULT_KEEP)
  if (!spec || spec.seconds === null || spec.seconds === undefined)
    return null
  return Number(nowSecs) + Number(spec.seconds)
}

function applyDefaultKeep(entry, defaultKeep, nowSecs) {
  // Apply before normalize defaults keep to 1d, so config default_keep wins
  // when the incoming payload omitted keep.
  if (!entry || typeof entry !== "object")
    return null
  var incoming = {}
  for (var k in entry) {
    if (Object.prototype.hasOwnProperty.call(entry, k))
      incoming[k] = entry[k]
  }
  var hadKeep = incoming.keep !== undefined && incoming.keep !== null && String(incoming.keep).length > 0
  if (!hadKeep)
    incoming.keep = keepByKey(defaultKeep) ? String(defaultKeep) : DEFAULT_KEEP
  else if (!keepByKey(incoming.keep))
    incoming.keep = keepByKey(defaultKeep) ? String(defaultKeep) : DEFAULT_KEEP

  var e = normalizeEntry(incoming)
  if (!e)
    return null
  if (e.keep_until === undefined) {
    var until = keepUntilFrom(e.keep, nowSecs || (Date.now() / 1000))
    e.keep_until = until === null ? null : until
  }
  return e
}

function isExpired(entry, nowSecs) {
  var e = normalizeEntry(entry)
  if (!e)
    return true
  if (e.keep_until === null || e.keep_until === undefined)
    return false
  var until = Number(e.keep_until)
  if (isNaN(until))
    return false
  return Number(nowSecs) >= until
}

function normalizeEntry(value) {
  if (!value || typeof value !== "object")
    return null

  var type = String(value.type || "")
  if (type === "text") {
    var text = String(value.text || "")
    if (!text.trim().length)
      return null
    var te = {
      type: "text",
      text: text,
      hash: String(value.hash || ""),
      ts: Number(value.ts || 0),
      kind: String(value.kind || "Text"),
      keep: String(value.keep || DEFAULT_KEEP),
      chars: text.length
    }
    if (value.keep_until === null)
      te.keep_until = null
    else if (value.keep_until !== undefined)
      te.keep_until = Number(value.keep_until)
    else
      te.keep_until = keepUntilFrom(te.keep, te.ts || (Date.now() / 1000))
    return te
  }

  if (type === "image") {
    var path = String(value.path || "")
    if (!path)
      return null
    var ie = {
      type: "image",
      path: path,
      mime: String(value.mime || "image/png"),
      hash: String(value.hash || ""),
      ts: Number(value.ts || 0),
      kind: String(value.kind || "Image"),
      keep: String(value.keep || DEFAULT_KEEP),
      chars: 0
    }
    if (value.keep_until === null)
      ie.keep_until = null
    else if (value.keep_until !== undefined)
      ie.keep_until = Number(value.keep_until)
    else
      ie.keep_until = keepUntilFrom(ie.keep, ie.ts || (Date.now() / 1000))
    return ie
  }

  return null
}

function entryKey(entry) {
  if (!entry)
    return ""
  if (entry.hash)
    return String(entry.hash)
  if (entry.type === "image")
    return "image:" + String(entry.path || "")
  return "text:" + String(entry.text || "")
}

function parseHistory(raw) {
  try {
    var parsed = JSON.parse(String(raw || "[]"))
    var next = []
    if (!Array.isArray(parsed))
      return next
    for (var i = 0; i < parsed.length; i++) {
      var entry = normalizeEntry(parsed[i])
      if (entry)
        next.push(entry)
    }
    return next
  } catch (e) {
    return []
  }
}

function addEntry(history, entry, limit, defaultKeep) {
  var now = Date.now() / 1000
  var normalized = applyDefaultKeep(entry, defaultKeep || DEFAULT_KEEP, now)
  var max = limit === undefined || limit === null ? DEFAULT_MAX_ITEMS : Number(limit)
  if (isNaN(max) || max < 1)
    max = DEFAULT_MAX_ITEMS
  if (!normalized)
    return Array.isArray(history) ? visibleHistory(history, now).slice(0, max) : []

  var key = entryKey(normalized)
  var next = [normalized]
  var values = Array.isArray(history) ? history : []
  for (var i = 0; i < values.length && next.length < max; i++) {
    var existing = normalizeEntry(values[i])
    if (!existing || entryKey(existing) === key)
      continue
    if (isExpired(existing, now))
      continue
    next.push(existing)
  }
  return next
}

function removeEntryAt(history, index) {
  var values = Array.isArray(history) ? history.slice() : []
  var target = Number(index)
  if (isNaN(target) || target < 0 || target >= values.length)
    return values
  values.splice(target, 1)
  return values
}

// Image paths present in oldHistory but not in newHistory (expiry, delete, cap).
function imagePathsRemoved(oldHistory, newHistory) {
  var kept = {}
  var newList = Array.isArray(newHistory) ? newHistory : []
  for (var i = 0; i < newList.length; i++) {
    var entry = normalizeEntry(newList[i])
    if (entry && entry.type === "image" && entry.path)
      kept[String(entry.path)] = true
  }
  var out = []
  var oldList = Array.isArray(oldHistory) ? oldHistory : []
  for (var j = 0; j < oldList.length; j++) {
    var old = normalizeEntry(oldList[j])
    if (old && old.type === "image" && old.path && !kept[String(old.path)])
      out.push(String(old.path))
  }
  return out
}

function touchEntryAt(history, index, nowSecs) {
  var values = Array.isArray(history) ? history.slice() : []
  var target = Number(index)
  if (isNaN(target) || target < 0 || target >= values.length)
    return values
  var entry = normalizeEntry(values[target])
  if (!entry)
    return values
  entry.ts = Number(nowSecs || (Date.now() / 1000))
  values.splice(target, 1)
  values.unshift(entry)
  return values
}

function updateEntryAt(history, index, mutator) {
  var values = Array.isArray(history) ? history.slice() : []
  var target = Number(index)
  if (isNaN(target) || target < 0 || target >= values.length)
    return values
  var entry = normalizeEntry(values[target])
  if (!entry)
    return values
  var updated = mutator(entry)
  if (!updated)
    return values
  values[target] = normalizeEntry(updated) || entry
  return values
}

function cycleKeepAt(history, index, nowSecs) {
  return updateEntryAt(history, index, function (entry) {
    var nxt = nextKeep(entry.keep)
    entry.keep = nxt.key
    entry.keep_until = keepUntilFrom(nxt.key, nowSecs || (Date.now() / 1000))
    return entry
  })
}

function renameKindAt(history, index, kind) {
  return updateEntryAt(history, index, function (entry) {
    var label = String(kind || "").trim()
    if (!label.length)
      return entry
    entry.kind = label.slice(0, 32)
    return entry
  })
}

function parseEntryJson(line) {
  var raw = String(line || "").trim()
  if (!raw)
    return null
  try {
    return normalizeEntry(JSON.parse(raw))
  } catch (e) {
    return null
  }
}

function previewText(entry) {
  if (!entry)
    return ""
  if (entry.type === "image")
    return "Image"
  return String(entry.text || "").replace(/\s+/g, " ")
}

function keepLabel(keep) {
  var spec = keepByKey(keep)
  return spec ? spec.label : String(keep || "")
}

function charLabel(entry) {
  if (!entry || entry.type === "image")
    return ""
  var n = entry.chars !== undefined ? Number(entry.chars) : String(entry.text || "").length
  if (isNaN(n))
    n = 0
  return n === 1 ? "1 character" : n + " characters"
}

function matchesFilter(entry, filter) {
  var needle = String(filter || "").trim().toLowerCase()
  if (!needle)
    return true
  if (entry.type === "image")
    return "image".indexOf(needle) >= 0 || String(entry.kind || "").toLowerCase().indexOf(needle) >= 0
  return String(entry.text || "").toLowerCase().indexOf(needle) >= 0
    || String(entry.kind || "").toLowerCase().indexOf(needle) >= 0
}

function visibleHistory(history, nowSecs) {
  var values = Array.isArray(history) ? history : []
  var now = Number(nowSecs || (Date.now() / 1000))
  var out = []
  for (var i = 0; i < values.length; i++) {
    var entry = normalizeEntry(values[i])
    if (!entry || isExpired(entry, now))
      continue
    out.push(entry)
  }
  return out
}

function displayRows(history, filter, limit, nowSecs) {
  var now = Number(nowSecs || (Date.now() / 1000))
  var values = Array.isArray(history) ? history : []
  var max = limit === undefined || limit === null ? 40 : Number(limit)
  if (isNaN(max) || max < 1)
    max = 40
  var rows = []
  for (var i = 0; i < values.length && rows.length < max; i++) {
    var entry = normalizeEntry(values[i])
    if (!entry || isExpired(entry, now) || !matchesFilter(entry, filter))
      continue
    rows.push({
      entryType: entry.type,
      fullText: entry.type === "text" ? entry.text : "",
      previewText: previewText(entry),
      path: entry.type === "image" ? entry.path : "",
      mime: entry.mime || "",
      hash: entry.hash || "",
      kind: entry.kind || (entry.type === "image" ? "Image" : "Text"),
      keep: entry.keep || DEFAULT_KEEP,
      keepLabel: keepLabel(entry.keep || DEFAULT_KEEP),
      chars: entry.type === "text" ? (entry.chars || String(entry.text || "").length) : 0,
      charLabel: charLabel(entry),
      ts: entry.ts || 0,
      historyIndex: i
    })
  }
  return rows
}

function ageLabel(ts, nowSecs) {
  var t = Number(ts || 0)
  var now = Number(nowSecs || 0)
  if (!t || !now || now < t)
    return ""
  var delta = Math.max(0, Math.floor(now - t))
  if (delta < 12)
    return "just now"
  if (delta < 60)
    return delta === 1 ? "1 second ago" : delta + " seconds ago"
  if (delta < 3600) {
    var minutes = Math.floor(delta / 60)
    return minutes === 1 ? "1 minute ago" : minutes + " minutes ago"
  }
  if (delta < 86400) {
    var hours = Math.floor(delta / 3600)
    return hours === 1 ? "1 hour ago" : hours + " hours ago"
  }
  var days = Math.floor(delta / 86400)
  return days === 1 ? "1 day ago" : days + " days ago"
}
