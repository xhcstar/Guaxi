import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const projectRoot = process.cwd();
const androidRoot = join(projectRoot, 'src-tauri', 'gen', 'android');
const manifestPath = join(androidRoot, 'app', 'src', 'main', 'AndroidManifest.xml');
const optional = process.argv.includes('--optional');

function fail(message) {
  console.error(`[capture-android-share] ${message}`);
  process.exit(1);
}

function read(path) {
  return readFileSync(path, 'utf8');
}

function write(path, content) {
  writeFileSync(path, content, 'utf8');
}

function findMainActivity() {
  const candidates = [
    join(androidRoot, 'app', 'src', 'main', 'java', 'com', 'guaxi', 'capture', 'MainActivity.kt'),
    join(androidRoot, 'app', 'src', 'main', 'kotlin', 'com', 'guaxi', 'capture', 'MainActivity.kt'),
  ];
  const hit = candidates.find((candidate) => existsSync(candidate));
  if (!hit) fail('MainActivity.kt not found. Run npm run android:init first.');
  return hit;
}

function patchManifest() {
  if (!existsSync(manifestPath)) fail('AndroidManifest.xml not found. Run npm run android:init first.');

  const marker = '<!-- Guaxi Capture share target -->';
  let manifest = read(manifestPath);
  if (manifest.includes(marker)) {
    console.info('[capture-android-share] manifest already patched');
    return;
  }

  const shareFilters = `
        ${marker}
        <intent-filter>
          <action android:name="android.intent.action.SEND" />
          <category android:name="android.intent.category.DEFAULT" />
          <data android:mimeType="text/*" />
        </intent-filter>
        <intent-filter>
          <action android:name="android.intent.action.SEND" />
          <category android:name="android.intent.category.DEFAULT" />
          <data android:mimeType="image/*" />
        </intent-filter>
        <intent-filter>
          <action android:name="android.intent.action.SEND_MULTIPLE" />
          <category android:name="android.intent.category.DEFAULT" />
          <data android:mimeType="image/*" />
        </intent-filter>`;

  const patchedManifest = manifest.replace(
    /(<activity\b(?=[^>]*android:name="(?:\.MainActivity|com\.guaxi\.capture\.MainActivity)")(?=[^>]*android:exported="true")[^>]*>[\s\S]*?)(\s*<\/activity>)/,
    `$1${shareFilters}$2`,
  );
  if (patchedManifest === manifest) {
    fail('MainActivity entry not found in AndroidManifest.xml; patch the share intent filters manually.');
  }

  manifest = patchedManifest;
  write(manifestPath, manifest);
  console.info('[capture-android-share] manifest patched');
}

function patchMainActivity(mainActivityPath) {
  let source = read(mainActivityPath);
  const packageLine = source.match(/^package\s+([\w.]+)/m);
  if (!packageLine) fail('Cannot determine MainActivity package.');
  const packageName = packageLine[1];

  source = ensureImport(source, packageLine[0], 'android.os.Bundle');
  source = ensureImport(source, packageLine[0], 'android.content.Intent');

  if (source.includes('pluginManager.load(null, "guaxi-share-intent"')) {
    if (!source.includes('override fun onNewIntent(intent: Intent)')) {
      source = source.replace(
        /\n}\s*$/,
        `\n\n  override fun onNewIntent(intent: Intent) {\n    super.onNewIntent(intent)\n    setIntent(intent)\n    pluginManager.onNewIntent(intent)\n  }\n}\n`,
      );
      write(mainActivityPath, source);
      console.info('[capture-android-share] MainActivity onNewIntent patched');
    }
    console.info('[capture-android-share] MainActivity already patched');
  } else if (/class\s+MainActivity\s*:\s*TauriActivity\(\)\s*$/.test(source.trim())) {
    source = source.replace(
      /class\s+MainActivity\s*:\s*TauriActivity\(\)\s*$/,
      `class MainActivity : TauriActivity() {\n  override fun onCreate(savedInstanceState: Bundle?) {\n    pluginManager.load(null, "guaxi-share-intent", ShareIntentPlugin(this), "{}")\n    super.onCreate(savedInstanceState)\n    intent?.let { pluginManager.onNewIntent(it) }\n  }\n\n  override fun onNewIntent(intent: Intent) {\n    super.onNewIntent(intent)\n    setIntent(intent)\n    pluginManager.onNewIntent(intent)\n  }\n}\n`,
    );
    write(mainActivityPath, source);
    console.info('[capture-android-share] MainActivity patched');
  } else if (/class\s+MainActivity\s*:\s*TauriActivity\(\)\s*\{/.test(source)) {
    source = patchExistingMainActivityClass(source);
    write(mainActivityPath, source);
    console.info('[capture-android-share] existing MainActivity class patched');
  } else {
    fail('MainActivity.kt shape is not the default Tauri form; patch it manually with ShareIntentPlugin registration.');
  }

  writeSharePlugin(dirname(mainActivityPath), packageName);
}

function ensureImport(source, packageDeclaration, importName) {
  if (source.includes(`import ${importName}`)) return source;

  const importMatches = [...source.matchAll(/^import\s+.+$/gm)];
  if (importMatches.length) {
    const lastImport = importMatches[importMatches.length - 1];
    return `${source.slice(0, lastImport.index + lastImport[0].length)}\nimport ${importName}${source.slice(lastImport.index + lastImport[0].length)}`;
  }

  return source.replace(packageDeclaration, `${packageDeclaration}\n\nimport ${importName}`);
}

function patchExistingMainActivityClass(source) {
  let patched = source;

  // 设计说明：
  // Tauri Android 生成工程可能已经被其它脚本补过，MainActivity 不一定还是一行默认类。
  // 这里按已有结构增量注入插件注册逻辑，避免每次重新生成/重复补丁时都要求手工修 Kotlin。
  if (/override\s+fun\s+onCreate\s*\(\s*savedInstanceState:\s*Bundle\?\s*\)\s*\{/.test(patched)) {
    patched = patched.replace(
      /(override\s+fun\s+onCreate\s*\(\s*savedInstanceState:\s*Bundle\?\s*\)\s*\{\s*)/,
      `$1\n    pluginManager.load(null, "guaxi-share-intent", ShareIntentPlugin(this), "{}")`,
    );
    if (!patched.includes('intent?.let { pluginManager.onNewIntent(it) }')) {
      patched = patched.replace(
        /(super\.onCreate\s*\(\s*savedInstanceState\s*\)\s*)/,
        `$1\n    intent?.let { pluginManager.onNewIntent(it) }`,
      );
    }
  } else {
    patched = insertBeforeMainActivityEnd(
      patched,
      `\n  override fun onCreate(savedInstanceState: Bundle?) {\n    pluginManager.load(null, "guaxi-share-intent", ShareIntentPlugin(this), "{}")\n    super.onCreate(savedInstanceState)\n    intent?.let { pluginManager.onNewIntent(it) }\n  }\n`,
    );
  }

  if (!patched.includes('override fun onNewIntent(intent: Intent)')) {
    patched = insertBeforeMainActivityEnd(
      patched,
      `\n  override fun onNewIntent(intent: Intent) {\n    super.onNewIntent(intent)\n    setIntent(intent)\n    pluginManager.onNewIntent(intent)\n  }\n`,
    );
  }

  return patched;
}

function insertBeforeMainActivityEnd(source, content) {
  const lastBrace = source.lastIndexOf('}');
  if (lastBrace < 0) {
    fail('MainActivity.kt has no class closing brace.');
  }
  return `${source.slice(0, lastBrace)}${content}${source.slice(lastBrace)}`;
}

function writeSharePlugin(activityDir, packageName) {
  const pluginPath = join(activityDir, 'ShareIntentPlugin.kt');
  const source = `package ${packageName}

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Base64
import android.util.Log
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray

@TauriPlugin
class ShareIntentPlugin(private val hostActivity: Activity) : Plugin(hostActivity) {
  private var pendingPayload: JSObject? = null
  private var appWebView: WebView? = null

  override fun load(webView: WebView) {
    super.load(webView)
    appWebView = webView
    captureIntent(hostActivity.intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    captureIntent(intent)
  }

  @Command
  fun readPendingShare(invoke: Invoke) {
    val payload = pendingPayload
    pendingPayload = null
    Log.i("GuaxiShareIntent", "readPendingShare hasPayload=" + (payload != null))
    invoke.resolve(payload)
  }

  private fun captureIntent(intent: Intent?) {
    if (intent == null) return
    val action = intent.action ?: return
    if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) return

    val payload = JSObject()
    val text = intent.getStringExtra(Intent.EXTRA_TEXT).orEmpty()
    val html = intent.getStringExtra(Intent.EXTRA_HTML_TEXT).orEmpty()
    val title = intent.getStringExtra(Intent.EXTRA_TITLE)
      ?: intent.getStringExtra(Intent.EXTRA_SUBJECT)
      ?: "Android Share"

    payload.put("title", title)
    payload.put("sourceApp", "android-share")
    payload.put("description", text)
    payload.put("text", text)
    payload.put("sharedText", text)
    payload.put("html", html)
    extractFirstUrl(text)?.let { payload.put("sourceUrl", it) }

    val assets = JSONArray()
    val uris = mutableListOf<Uri>()
    if (action == Intent.ACTION_SEND_MULTIPLE) {
      intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)?.let { uris.addAll(it) }
    } else {
      intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.let { uris.add(it) }
    }

    for (uri in uris) {
      readAsset(uri)?.let { assets.put(it) }
    }

    if (assets.length() > 0) {
      payload.put("assets", assets)
      payload.put("sourceApp", "camera")
      payload.put("tagsText", "capture,ocr,camera")
    }

    pendingPayload = payload
    Log.i(
      "GuaxiShareIntent",
      "captured share title=" + title + " url=" + payload.optString("sourceUrl") + " assets=" + assets.length()
    )
    emitPayload(payload)
  }

  private fun emitPayload(payload: JSObject) {
    val webView = appWebView ?: return
    val script = """
      window.__GUAXI_CAPTURE_LAST_ANDROID_SHARE__ = \${payload};
      window.dispatchEvent(new CustomEvent('guaxi-android-share-intent', { detail: \${payload} }));
    """.trimIndent()
    webView.post {
      webView.evaluateJavascript(script, null)
    }
  }

  private fun readAsset(uri: Uri): JSObject? {
    return try {
      val resolver = hostActivity.contentResolver
      val bytes = resolver.openInputStream(uri)?.use { it.readBytes() } ?: return null
      JSObject()
        .put("name", displayName(uri))
        .put("mime", resolver.getType(uri) ?: "application/octet-stream")
        .put("dataBase64", Base64.encodeToString(bytes, Base64.NO_WRAP))
    } catch (_: Exception) {
      null
    }
  }

  private fun displayName(uri: Uri): String {
    val resolver = hostActivity.contentResolver
    resolver.query(uri, null, null, null, null)?.use { cursor ->
      val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
      if (index >= 0 && cursor.moveToFirst()) {
        return cursor.getString(index)
      }
    }
    return uri.lastPathSegment ?: "shared-image"
  }

  private fun extractFirstUrl(text: String): String? {
    return Regex("https?://\\\\S+").find(text)?.value
  }
}
`;

  write(pluginPath, source);
  console.info(`[capture-android-share] plugin written: ${pluginPath}`);
}

if (!existsSync(androidRoot)) {
  if (optional) {
    console.info('[capture-android-share] Android generated project not found, skip optional patch');
    process.exit(0);
  }
  fail('Android generated project not found. Run npm run android:init first.');
}

patchManifest();
patchMainActivity(findMainActivity());
console.info('[capture-android-share] Android share target configured');
