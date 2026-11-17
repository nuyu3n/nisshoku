package app.nisshoku.android.ui

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.runtime.CompositionLocalProvider
import app.nisshoku.android.tr
import java.text.BreakIterator

// Manuscript paper (原稿用紙): the text placed cell by cell, written top to
// bottom and right to left, by the usual rules (the web app's lib/genko.ts):
// paragraphs start one cell down (not before 「『（); two half-width
// characters stand side by side in a cell (縦中横); words lie on their side;
// 、。 and closing brackets never start a line but hang below the last cell;
// ruby (|漢字《かんじ》) sits beside its characters.

enum class CellKind { Plain, Tcy, Up, Rot }

data class Cell(val t: String, val k: CellKind = CellKind.Plain)

data class RubySpan(val at: Int, val len: Int, val text: String)

class GLine(val cells: MutableList<Cell> = mutableListOf(), val ruby: MutableList<RubySpan> = mutableListOf(), val hang: MutableList<Cell> = mutableListOf())

private class GUnit(val cells: List<Cell>, val ruby: String? = null, val keep: Boolean = false)

private val RUBY = Regex("""[|｜]([^|｜《》\n]+)《([^《》\n]+)》|([\p{IsHan}々〆ヵヶ]+)《([^《》\n]+)》""")
private const val HANG = "、。，．」』）】〕〉》〙〗’”"
private const val NO_INDENT = "「『（〈《【〔“‘"
private val WIDE = mapOf(',' to "、", '.' to "．", '!' to "！", '?' to "？", '(' to "（", ')' to "）", '[' to "［", ']' to "］", '-' to "－", '~' to "〜", '\'' to "’", '"' to "”")

private fun graphemes(s: String): List<String> {
    val it = BreakIterator.getCharacterInstance()
    it.setText(s)
    val out = mutableListOf<String>()
    var start = it.first()
    var end = it.next()
    while (end != BreakIterator.DONE) {
        out += s.substring(start, end)
        start = end
        end = it.next()
    }
    return out
}

private fun asciiUnits(raw: String, out: MutableList<GUnit>) {
    val run = raw.replace(Regex(" {2,}"), " ").trim()
    if (run.isEmpty()) {
        out += GUnit(listOf(Cell("　")))
        return
    }
    fun one(c: Char): Cell = when {
        c.isDigit() -> Cell(c.toString(), CellKind.Tcy)
        c in 'A'..'Z' || c in 'a'..'z' -> Cell(c.toString(), CellKind.Up)
        else -> Cell(WIDE[c] ?: (c + 0xfee0).toString())
    }
    when {
        run.length == 1 -> out += GUnit(listOf(one(run[0])))
        run.length == 2 && !run.contains(' ') -> out += GUnit(listOf(Cell(run, CellKind.Tcy)))
        run.all { it.isDigit() } || Regex("""^[0-9]+([-/.:][0-9]+)+$""").matches(run) || (run.all { it in 'A'..'Z' } && run.length <= 5) ->
            run.forEach { out += GUnit(listOf(one(it))) }
        else -> {
            val cells = run.chunked(2).map { Cell(it, CellKind.Rot) }
            out += GUnit(cells, keep = cells.size <= 4)
        }
    }
}

private fun plainUnits(s: String, out: MutableList<GUnit>) {
    val ascii = StringBuilder()
    fun flush() {
        if (ascii.isNotEmpty()) asciiUnits(ascii.toString(), out)
        ascii.clear()
    }
    for (g in graphemes(s)) {
        if (g.length == 1 && g[0].code in 0x20..0x7e) ascii.append(g)
        else {
            flush()
            out += GUnit(listOf(Cell(if (g == "\t") "　" else g)))
        }
    }
    flush()
}

private fun units(p: String): List<GUnit> {
    val out = mutableListOf<GUnit>()
    var last = 0
    for (m in RUBY.findAll(p)) {
        plainUnits(p.substring(last, m.range.first), out)
        val base = mutableListOf<GUnit>()
        plainUnits(m.groups[1]?.value ?: m.groups[3]!!.value, base)
        out += GUnit(base.flatMap { it.cells }, m.groups[2]?.value ?: m.groups[4]!!.value, keep = true)
        last = m.range.last + 1
    }
    plainUnits(p.substring(last), out)
    return out
}

private fun splitRuby(text: String, sizes: List<Int>): List<String> {
    val chars = graphemes(text)
    val total = sizes.sum()
    val out = mutableListOf<String>()
    var used = 0
    var acc = 0
    for (n in sizes) {
        acc += n
        val end = Math.round(chars.size * acc.toDouble() / total).toInt()
        out += chars.subList(used, end).joinToString("")
        used = end
    }
    return out
}

/** Lines of `cols` cells; an empty paragraph is an empty line. */
fun layoutLines(paragraphs: List<String>, cols: Int, indent: Boolean = true): List<GLine> {
    val lines = mutableListOf<GLine>()
    for (p in paragraphs) {
        val us = units(p.trim())
        var line = GLine()
        fun newLine() {
            lines += line
            line = GLine()
        }
        val first = us.firstOrNull()?.cells?.firstOrNull()
        if (indent && first != null && first.t !in NO_INDENT.map { it.toString() }) line.cells += Cell("　")
        for (u in us) {
            val n = u.cells.size
            if (n == 0) continue
            if (line.cells.size + n > cols) {
                if (n == 1 && u.ruby == null && u.cells[0].t in HANG.map { it.toString() } && line.cells.size == cols && line.hang.size < 2) {
                    line.hang += u.cells[0]
                    continue
                }
                if ((n == 1 || u.keep) && n <= cols) newLine()
            }
            if (n == 1 && u.ruby == null) {
                line.cells += u.cells[0]
                continue
            }
            if (u.ruby == null) {
                for (c in u.cells) {
                    if (line.cells.size >= cols) newLine()
                    line.cells += c
                }
                continue
            }
            val pieces = mutableListOf<Triple<GLine, Int, Int>>()
            var start = line.cells.size
            for (c in u.cells) {
                if (line.cells.size >= cols) {
                    pieces += Triple(line, start, line.cells.size - start)
                    newLine()
                    start = 0
                }
                line.cells += c
            }
            pieces += Triple(line, start, line.cells.size - start)
            val texts = splitRuby(u.ruby, pieces.map { it.third })
            pieces.forEachIndexed { i, (l, at, len) -> if (texts[i].isNotEmpty()) l.ruby += RubySpan(at, len, texts[i]) }
        }
        lines += line
    }
    return lines
}

/** The text of a note as paragraphs for the paper: Markdown marks, images and code taken out. */
fun manuscriptParagraphs(body: String): List<String> {
    val out = mutableListOf<String>()
    var fenced = false
    for (raw in body.split('\n')) {
        if (raw.trimStart().startsWith("```")) {
            fenced = !fenced
            continue
        }
        if (fenced || raw.isBlank()) continue
        if (Regex("""^\s*([-*_])(\s*\1){2,}\s*$""").matches(raw)) {
            out += "　　　　＊"
            continue
        }
        var s = raw
            .replace(Regex("""^\s*(#{1,6}|[-*+]|\d+[.)]|>)[ \t]+(\[[ xX]\][ \t]+)?"""), "")
            .replace(Regex("""!\[[^\]]*]\([^)]*\)"""), "")
            .replace(Regex("""\[([^\]]+)]\([^)]*\)"""), "$1")
            .replace(Regex("""(\*\*|__|~~|`)"""), "")
            .replace(Regex("""(?<![\w*])[*_](?=\S)(.+?)(?<=\S)[*_](?![\w*])"""), "$1")
            .replace(Regex("""<[^>]+>"""), "")
        s = s.trimEnd()
        if (s.isNotBlank()) out += s
    }
    return out
}

/** The paper's ink (the `genkoInk` preference). */
private fun ink(name: String, accent: Color): Color = when (name) {
    "vermilion" -> Color(0xFFC8392B)
    "green" -> Color(0xFF3B7A57)
    "indigo" -> Color(0xFF3F5EA8)
    "sumi" -> Color(0xFF4A4540)
    else -> accent
}

/**
 * The note on manuscript paper, a page at a time, read right to left
 * (swipe right for the next page).
 */
@Composable
fun GenkoPreview(title: String, body: String, cols: Int, rows: Int, inkName: String, onDismiss: () -> Unit) {
    val lines = remember(title, body, cols, rows) {
        val head = mutableListOf<GLine>()
        if (title.isNotBlank()) {
            for (l in layoutLines(listOf(title), maxOf(1, cols - 3), indent = false)) {
                head += GLine(
                    (List(3) { Cell("　") } + l.cells).toMutableList(),
                    l.ruby.map { it.copy(at = it.at + 3) }.toMutableList(),
                    l.hang,
                )
            }
            head += GLine()
        }
        head + layoutLines(manuscriptParagraphs(body), cols)
    }
    // A phone shows half a sheet at a time (the paper's halves, either side of its fold).
    val half = maxOf(1, (rows + 1) / 2)
    val pages = remember(lines, rows) { lines.chunked(half).ifEmpty { listOf(emptyList()) } }
    val pager = rememberPagerState { pages.size }
    val paper = Color(0xFFFBF8F1)
    val text = Color(0xFF2C2A26)
    val gridInk = ink(inkName, MaterialTheme.colorScheme.primary)
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false)) {
        Surface(color = Color(0xFF3A3833), modifier = Modifier.fillMaxSize()) {
            Column(Modifier.statusBarsPadding().navigationBarsPadding()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = onDismiss) { Icon(Icons.AutoMirrored.Outlined.ArrowBack, tr("閉じる", "Close"), tint = Color.White) }
                    Text(tr("縦書きプレビュー", "Vertical preview"), color = Color.White, style = MaterialTheme.typography.titleMedium, modifier = Modifier.weight(1f))
                    Text(
                        tr("${pager.currentPage / 2 + 1}枚目・${if (pager.currentPage % 2 == 0) "右" else "左"}（全${(pages.size + 1) / 2}枚）", "Sheet ${pager.currentPage / 2 + 1} ${if (pager.currentPage % 2 == 0) "right" else "left"} of ${(pages.size + 1) / 2}"),
                        color = Color.White.copy(alpha = 0.8f),
                        style = MaterialTheme.typography.labelLarge,
                        modifier = Modifier.padding(end = 16.dp),
                    )
                }
                // Right to left: the first page at the right, the next one swiped in from the left.
                CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
                    HorizontalPager(state = pager, modifier = Modifier.fillMaxSize()) { i ->
                        CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Ltr) {
                            BoxWithConstraints(Modifier.fillMaxSize().padding(12.dp), contentAlignment = Alignment.Center) {
                                // Cells: `rows` lines across (plus room for ruby), `cols` cells down (plus the hanging row).
                                val cell = minOf(maxWidth / (half * 1.45f + 1f), maxHeight / (cols + 2f))
                                Box(Modifier.background(paper).padding(cell * 0.5f)) {
                                    Page(pages[i], cols, half, cell, gridInk, text)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun Page(lines: List<GLine>, cols: Int, rows: Int, cell: androidx.compose.ui.unit.Dp, gridInk: Color, ink: Color) {
    val gap = cell * 0.45f
    Canvas(Modifier.size(width = (cell + gap) * rows, height = cell * (cols + 1))) {
        val c = cell.toPx()
        val g = gap.toPx()
        val width = size.width
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = ink.toArgb()
            typeface = Typeface.create(Typeface.SERIF, Typeface.NORMAL)
            textAlign = Paint.Align.CENTER
            fontFeatureSettings = "'vert' 1, 'vrt2' 1"
        }
        val small = Paint(paint).apply { fontFeatureSettings = null }
        // The grid: a column of cells for each line, right to left, a gap (for ruby) beside each.
        for (r in 0 until rows) {
            val x = width - (r + 1) * (c + g) + g
            for (k in 0 until cols) {
                drawRect(gridInk.copy(alpha = 0.55f), topLeft = androidx.compose.ui.geometry.Offset(x, k * c), size = androidx.compose.ui.geometry.Size(c, c), style = Stroke(1f))
            }
        }
        drawIntoCanvas { canvas ->
            val nc = canvas.nativeCanvas
            lines.forEachIndexed { r, line ->
                val x = width - (r + 1) * (c + g) + g
                val cx = x + c / 2
                fun drawCell(cellAt: Cell, k: Int) {
                    val cy = k * c + c / 2
                    when (cellAt.k) {
                        CellKind.Tcy -> {
                            small.textSize = c * if (cellAt.t.length > 1) 0.55f else 0.72f
                            nc.drawText(cellAt.t, cx, cy + small.textSize * 0.36f, small)
                        }
                        CellKind.Up -> {
                            small.textSize = c * 0.72f
                            nc.drawText(cellAt.t, cx, cy + small.textSize * 0.36f, small)
                        }
                        CellKind.Rot -> {
                            small.textSize = c * 0.62f
                            nc.save()
                            nc.rotate(90f, cx, cy)
                            nc.drawText(cellAt.t, cx, cy + small.textSize * 0.36f, small)
                            nc.restore()
                        }
                        CellKind.Plain -> {
                            paint.textSize = c * 0.78f
                            nc.drawText(cellAt.t, cx, cy + paint.textSize * 0.36f, paint)
                        }
                    }
                }
                line.cells.forEachIndexed { k, cl -> drawCell(cl, k) }
                line.hang.forEachIndexed { k, cl -> drawCell(cl, cols + k) }
                for (rb in line.ruby) {
                    val ruby = Paint(paint).apply { textSize = g * 0.82f }
                    val chars = graphemes(rb.text)
                    val top = rb.at * c
                    val span = rb.len * c
                    val step = maxOf(ruby.textSize, span / maxOf(1, chars.size))
                    val start = top + (span - step * chars.size) / 2
                    chars.forEachIndexed { i, ch ->
                        nc.drawText(ch, x + c + g / 2, start + step * i + step / 2 + ruby.textSize * 0.36f, ruby)
                    }
                }
            }
        }
    }
}
