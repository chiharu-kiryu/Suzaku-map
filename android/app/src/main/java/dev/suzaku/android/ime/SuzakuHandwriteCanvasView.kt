package dev.suzaku.android.ime

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View

data class StrokePoint(val x: Float, val y: Float)

class SuzakuHandwriteCanvasView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {
    private val pathPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#3C88D8")
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeJoin = Paint.Join.ROUND
        strokeWidth = 8f
    }

    private val guidePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#BCD0E8")
        style = Paint.Style.STROKE
        strokeWidth = 2f
    }

    private val paths = mutableListOf<Path>()
    private val strokes = mutableListOf<List<StrokePoint>>()
    private var currentPath = Path()
    private var currentStroke = mutableListOf<StrokePoint>()
    var onStrokeFinished: ((List<List<StrokePoint>>) -> Unit)? = null
    var onStrokeStarted: (() -> Unit)? = null

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        val midY = height / 2f
        canvas.drawLine(0f, midY, width.toFloat(), midY, guidePaint)
        paths.forEach { canvas.drawPath(it, pathPaint) }
        canvas.drawPath(currentPath, pathPaint)
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                parent?.requestDisallowInterceptTouchEvent(true)
                currentPath = Path().apply { moveTo(event.x, event.y) }
                currentStroke = mutableListOf(StrokePoint(event.x, event.y))
                onStrokeStarted?.invoke()
                invalidate()
                return true
            }
            MotionEvent.ACTION_MOVE -> {
                currentPath.lineTo(event.x, event.y)
                currentStroke.add(StrokePoint(event.x, event.y))
                invalidate()
                return true
            }
            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                currentPath.lineTo(event.x, event.y)
                currentStroke.add(StrokePoint(event.x, event.y))
                paths.add(currentPath)
                strokes.add(currentStroke.toList())
                currentPath = Path()
                currentStroke = mutableListOf()
                invalidate()
                onStrokeFinished?.invoke(strokes.map { it.toList() })
                return true
            }
        }
        return super.onTouchEvent(event)
    }

    fun clearCanvas() {
        paths.clear()
        strokes.clear()
        currentPath = Path()
        currentStroke.clear()
        invalidate()
    }

    fun strokeGroups(): List<List<StrokePoint>> = strokes.map { it.toList() }
}
