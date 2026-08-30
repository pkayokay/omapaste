#!/usr/bin/env node
// Headless parity checks for History.js + Config.js (QML .pragma library).
// Run: node tests/qml-parity.mjs

import fs from "node:fs"
import path from "node:path"
import { fileURLToPath } from "node:url"
import vm from "node:vm"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
let failed = 0

function assert( cond, msg ) {
  if ( !cond ) {
    failed++
    console.error( "FAIL:", msg )
  } else {
    console.log( "ok:", msg )
  }
}

function loadLibrary( file ) {
  const src = fs.readFileSync( path.join( root, file ), "utf8" )
    .replace( /^\.pragma library\s*/m, "" )
  const sandbox = { console }
  vm.createContext( sandbox )
  vm.runInContext( src + "\nthis.__exports = {" +
    "KEEP_PRESETS,DEFAULT_KEEP,DEFAULT_MAX_ITEMS," +
    "keepByKey,nextKeep,keepUntilFrom,applyDefaultKeep,isExpired," +
    "normalizeEntry,entryKey,parseHistory,addEntry,removeEntryAt,imagePathsRemoved," +
    "cycleKeepAt,renameKindAt,parseEntryJson,previewText,keepLabel," +
    "charLabel,matchesFilter,visibleHistory,displayRows,ageLabel" +
    "};", sandbox )
  return sandbox.__exports
}

function loadConfig( file ) {
  const src = fs.readFileSync( path.join( root, file ), "utf8" )
    .replace( /^\.pragma library\s*/m, "" )
  const sandbox = { console }
  vm.createContext( sandbox )
  vm.runInContext( src + "\nthis.__exports = { DEFAULTS, normalize, parse };", sandbox )
  return sandbox.__exports
}

const H = loadLibrary( "History.js" )
const C = loadConfig( "Config.js" )

// --- Config ---
assert( C.normalize( null ).default_keep === "1d", "config default_keep" )
assert( C.normalize( null ).max_bytes === 8000000, "config max_bytes default" )
assert( C.normalize( { max_bytes: 1024 } ).max_bytes === 1024, "config max_bytes override" )
assert( C.normalize( { default_keep: "7d", max_items: 50, paste_keys: "ctrl-v" } ).paste_keys === "ctrl-v", "config paste_keys" )
assert( C.normalize( { default_keep: "nope" } ).default_keep === "1d", "config rejects bad keep" )
assert( C.parse( "{ bad" ).max_items === 200, "config parse fallback" )

// --- Keep cycle ---
assert( H.nextKeep( "1h" ).key === "1d", "keep 1h→1d" )
assert( H.nextKeep( "1d" ).key === "7d", "keep 1d→7d" )
assert( H.nextKeep( "7d" ).key === "forever", "keep 7d→forever" )
assert( H.nextKeep( "forever" ).key === "1h", "keep forever→1h" )
assert( H.keepUntilFrom( "forever", 1000 ) === null, "forever keep_until null" )
assert( H.keepUntilFrom( "1h", 1000 ) === 4600, "1h keep_until" )

// --- Normalize / add / default_keep from config ---
// Use "now" near Date.now so addEntry's internal clock doesn't treat clips as expired.
const now = Math.floor( Date.now() / 1000 )
let hist = []
hist = H.addEntry( hist, { type: "text", text: "alpha", hash: "h1", ts: now }, 10, "7d" )
assert( hist.length === 1, "add first clip" )
assert( hist[0].keep === "7d", "new clip uses config default_keep=7d" )
assert( typeof hist[0].keep_until === "number" && hist[0].keep_until > now, "7d keep_until in future" )

assert( hist[0].chars === 5, "char count" )
assert( H.charLabel( hist[0] ) === "5c", "charLabel" )

hist = H.addEntry( hist, { type: "text", text: "alpha", hash: "h1", ts: now }, 10, "7d" )
assert( hist.length === 1, "dedupe by hash" )

hist = H.addEntry( hist, { type: "text", text: "beta", hash: "h2", ts: now }, 10, "1d" )
assert( hist[0].text === "beta", "newest first" )
assert( hist.length === 2, "two clips" )
assert( hist[1].keep === "7d", "older clip keeps prior default_keep" )

// Explicit keep on payload wins over config default
hist = H.addEntry( hist, { type: "text", text: "gamma", hash: "h3", ts: now, keep: "1h" }, 10, "7d" )
assert( hist[0].keep === "1h", "explicit keep wins over config default" )

// --- Expiry ---
const expired = H.normalizeEntry( {
  type: "text", text: "old", hash: "hexp", ts: now - 100,
  keep: "1h", keep_until: now - 1
} )
assert( H.isExpired( expired, now ), "expired clip detected" )
assert( H.visibleHistory( [ expired, hist[0] ], now ).length === 1, "visibleHistory hides expired" )
assert( H.displayRows( [ expired, ...hist ], "", 40, now ).length === hist.length, "displayRows skips expired" )

// --- Cycle keep ---
hist = H.cycleKeepAt( [ { type: "text", text: "c", hash: "hc", ts: now, keep: "1d" } ], 0, now )
assert( hist[0].keep === "7d", "cycle 1d→7d exact" )

// --- Rename ---
hist = H.renameKindAt( hist, 0, "  Code  " )
assert( hist[0].kind === "Code", "rename trims" )
hist = H.renameKindAt( hist, 0, "x".repeat( 40 ) )
assert( hist[0].kind.length === 32, "rename caps at 32" )

// --- Filter / search ---
assert( H.matchesFilter( hist[0], "x" ), "filter kind" )
assert( !H.matchesFilter( { type: "text", text: "zzz", kind: "Text" }, "nope" ), "filter miss" )
const withBeta = [
  { type: "text", text: "beta hello", hash: "hb", ts: now, keep: "1d" },
  { type: "text", text: "other", hash: "ho", ts: now, keep: "1d" }
]
assert( H.displayRows( withBeta, "beta", 40, now ).length === 1, "displayRows filter" )

// --- Remove ---
const n = withBeta.length
const afterRm = H.removeEntryAt( withBeta, 0 )
assert( afterRm.length === n - 1, "removeEntryAt" )

const imgPath = "/tmp/omapaste-test.png"
const imgEntry = H.normalizeEntry( { type: "image", path: imgPath, mime: "image/png", hash: "imgp", ts: now } )
assert( H.imagePathsRemoved( [ imgEntry ], [] ).indexOf( imgPath ) >= 0, "imagePathsRemoved drop" )
assert( H.imagePathsRemoved( [ imgEntry ], [ imgEntry ] ).length === 0, "imagePathsRemoved keep" )

// --- Image ---
const img = H.normalizeEntry( { type: "image", path: "/tmp/x.png", mime: "image/png", hash: "img1", ts: now } )
assert( img && img.kind === "Image" && img.chars === 0, "image normalize" )
assert( H.previewText( img ) === "Image", "image preview" )

// --- Age ---
assert( H.ageLabel( now - 30, now ) === "30s", "age seconds" )
assert( H.ageLabel( now - 120, now ) === "2m", "age minutes" )
assert( H.ageLabel( now - 7200, now ) === "2h", "age hours" )
assert( H.ageLabel( now - 90000, now ) === "1d", "age days" )

// --- parseHistory ---
assert( H.parseHistory( "not-json" ).length === 0, "parseHistory bad json" )
assert( H.parseHistory( JSON.stringify( [ { type: "text", text: "z", hash: "hz" } ] ) ).length === 1, "parseHistory ok" )

if ( failed ) {
  console.error( `\n${failed} failure(s)` )
  process.exit( 1 )
}
console.log( "\nAll History/Config parity checks passed." )
