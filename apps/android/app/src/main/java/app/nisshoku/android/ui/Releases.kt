package app.nisshoku.android.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.AutoAwesome
import androidx.compose.material.icons.outlined.SystemUpdate
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import app.nisshoku.android.engine.Release
import app.nisshoku.android.tr

/** What changed in a version (a section of CHANGELOG.md). */
data class ReleaseNotes(val version: String, val items: List<String>) {
    companion object {
        /** The versions of the CHANGELOG.md bundled with the app, newest first. */
        fun bundled(context: Context): List<ReleaseNotes> {
            val text = runCatching { context.assets.open("CHANGELOG.md").bufferedReader().use { it.readText() } }.getOrDefault("")
            val out = mutableListOf<ReleaseNotes>()
            var version: String? = null
            var items = mutableListOf<String>()
            fun flush() {
                version?.let { out += ReleaseNotes(it, items) }
            }
            for (line in text.lines()) {
                val head = Regex("^##\\s+(\\S+)").find(line)
                val item = Regex("^\\s*-\\s+(.*)").find(line)
                if (head != null) {
                    flush()
                    version = head.groupValues[1]
                    items = mutableListOf()
                } else if (item != null && version != null) {
                    items += item.groupValues[1].replace("**", "").replace("`", "")
                }
            }
            flush()
            return out
        }
    }
}

/** Whether version `a` is newer than `b` (`0.5.1` > `0.5.0`; a pre-release is older than its release). */
fun isNewer(a: String, b: String): Boolean {
    fun parts(v: String) = v.removePrefix("v").substringBefore('-').split('.').map { it.toIntOrNull() ?: 0 }
    val x = parts(a)
    val y = parts(b)
    for (i in 0 until maxOf(x.size, y.size)) {
        val d = x.getOrElse(i) { 0 } - y.getOrElse(i) { 0 }
        if (d != 0) return d > 0
    }
    return !a.contains('-') && b.contains('-')
}

@Composable
fun WhatsNewDialog(releases: List<ReleaseNotes>, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Outlined.AutoAwesome, null) },
        title = { Text(tr("新しくなったこと", "What's new")) },
        text = {
            Column(Modifier.heightIn(max = 420.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                releases.filter { it.items.isNotEmpty() }.forEach { release ->
                    Text(release.version, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
                    release.items.forEach { item ->
                        Row {
                            Text("•", color = MaterialTheme.colorScheme.outline)
                            Spacer(Modifier.width(8.dp))
                            Text(item, style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                    Spacer(Modifier.height(8.dp))
                }
            }
        },
        confirmButton = { Button(onClick = onDismiss) { Text(tr("閉じる", "Close")) } },
    )
}

/** A newer release: its notes, and the download of its package (the system installs it). */
@Composable
fun UpdateDialog(release: Release, onDismiss: () -> Unit) {
    val context = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        icon = { Icon(Icons.Outlined.SystemUpdate, null) },
        title = { Text(tr("バージョン ${release.version} があります", "Version ${release.version} is out")) },
        text = {
            Column(Modifier.heightIn(max = 360.dp).verticalScroll(rememberScrollState())) {
                Text(release.notes.replace("**", "").replace("`", "").trim(), style = MaterialTheme.typography.bodyMedium)
            }
        },
        confirmButton = {
            Button(onClick = {
                runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(release.apkUrl ?: release.url))) }
                onDismiss()
            }) { Text(tr("ダウンロード", "Download")) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("あとで", "Later")) } },
    )
}
