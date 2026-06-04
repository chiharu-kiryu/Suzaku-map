package dev.suzaku.android.ime

object SuzakuHandwriteRecognizer {
    fun candidateSeeds(strokes: List<List<StrokePoint>>): List<String> {
        if (strokes.isEmpty()) {
            return emptyList()
        }

        val points = strokes.flatten()
        val minX = points.minOf { it.x }
        val maxX = points.maxOf { it.x }
        val minY = points.minOf { it.y }
        val maxY = points.maxOf { it.y }
        val width = maxX - minX
        val height = maxY - minY
        val strokeCount = strokes.size
        val dense = points.size > 20

        val candidates = linkedSetOf<String>()
        when {
            strokeCount >= 3 && dense -> {
                candidates += "ni hao"
                candidates += "write text"
                candidates += "hello input"
            }
            width > height * 1.6f -> {
                candidates += "wave"
                candidates += "line"
                candidates += "write"
            }
            height > width * 1.4f -> {
                candidates += "tall"
                candidates += "stroke"
                candidates += "draw"
            }
            else -> {
                candidates += "word"
                candidates += "trace"
                candidates += "ni hao"
            }
        }

        candidates += "hello"
        candidates += "input"
        return candidates.take(5)
    }
}
