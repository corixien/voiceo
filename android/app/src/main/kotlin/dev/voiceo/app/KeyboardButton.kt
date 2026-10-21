package dev.voiceo.app

import android.content.Context
import android.graphics.Canvas
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.graphics.Shader
import android.view.View
import kotlin.math.PI
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sin

/** Key-sized floating button with the Voi mascot; shown above the keyboard, tap = toggle dictation. */
class KeyboardButton(ctx: Context) : View(ctx) {
    var state = "idle"
        set(v) { field = v; postInvalidateOnAnimation() }
    var level = 0f
    var unlocked = false
        set(v) { field = v; postInvalidateOnAnimation() }

    private val p = Paint(Paint.ANTI_ALIAS_FLAG)
    private val rect = RectF()
    private val body = Path()

    override fun onAttachedToWindow() { super.onAttachedToWindow(); postInvalidateOnAnimation() }

    override fun onDraw(c: Canvas) {
        val w = width.toFloat(); val h = height.toFloat()
        val t = System.currentTimeMillis()
        val active = state == "listening" || state == "thinking"

        // key background
        rect.set(0f, 0f, w, h)
        p.style = Paint.Style.FILL; p.shader = null
        p.color = if (state == "listening") 0xFF3A2A24.toInt() else 0xFF2B2A28.toInt()
        c.drawRoundRect(rect, w * 0.26f, w * 0.26f, p)
        p.style = Paint.Style.STROKE; p.strokeWidth = max(1f, w * 0.025f)
        p.color = if (unlocked) 0xCCE27B58.toInt() else 0x22FFFFFF
        if (unlocked) p.pathEffect = android.graphics.DashPathEffect(floatArrayOf(w * 0.1f, w * 0.07f), 0f)
        rect.inset(p.strokeWidth / 2, p.strokeWidth / 2)
        c.drawRoundRect(rect, w * 0.26f, w * 0.26f, p)
        p.pathEffect = null

        if (state == "listening") {
            val pulse = (0.5f + 0.5f * sin(t / 180.0 * PI / 2).toFloat()) * 0.4f + min(1f, level * 9f) * 0.6f
            p.style = Paint.Style.STROKE; p.strokeWidth = w * 0.05f
            p.color = (((90 + 140 * pulse).toInt().coerceIn(0, 255)) shl 24) or 0xE27B58
            rect.set(0f, 0f, w, h); rect.inset(w * 0.02f, w * 0.02f)
            c.drawRoundRect(rect, w * 0.25f, w * 0.25f, p)
        } else if (state == "thinking") {
            p.style = Paint.Style.STROKE; p.strokeWidth = w * 0.05f; p.color = 0xCCE27B58.toInt(); p.strokeCap = Paint.Cap.ROUND
            rect.set(0f, 0f, w, h); rect.inset(w * 0.04f, w * 0.04f)
            c.drawArc(rect, (t / 4 % 360).toFloat(), 90f, false, p)
        }

        // Voi (same shapes as src/mascot.js, drawn in a 60x52 box centered in the key)
        val s = w * 0.62f / 60f
        c.save()
        c.translate(w / 2f, h / 2f + w * 0.04f)
        c.scale(s, s)
        val happy = state == "done" || state == "learned"
        val talk = if (state == "listening") min(1f, level * 9f) else 0f
        val br = 1f + sin(t / 650.0).toFloat() * 0.018f + talk * 0.07f
        c.translate(0f, 26f); c.scale(1f / kotlin.math.sqrt(br), br); c.translate(0f, -26f)

        // antenna
        val wob = if (state == "listening") talk * 7f else if (state == "thinking") sin(t / 160.0).toFloat() * 3f else sin(t / 900.0).toFloat() * 1.5f
        p.style = Paint.Style.STROKE; p.strokeWidth = 3f; p.strokeCap = Paint.Cap.ROUND; p.color = 0xFFE2724F.toInt()
        val bx = -17f + wob * 0.6f; val by = -41f - kotlin.math.abs(wob) * 0.3f
        body.reset(); body.moveTo(-9f, -24f); body.quadTo(-13f, -34f, bx, by); c.drawPath(body, p)
        p.style = Paint.Style.FILL; p.color = 0xFFFFC9A8.toInt(); c.drawCircle(bx, by, 4.2f + talk * 1.6f, p)

        // body
        body.reset()
        rect.set(-30f, -26f, 30f, 26f)
        body.addRoundRect(rect, floatArrayOf(30f, 30f, 30f, 30f, 15f, 15f, 15f, 15f), Path.Direction.CW)
        p.shader = LinearGradient(0f, -26f, 0f, 26f, 0xFFF29A76.toInt(), 0xFFD96844.toInt(), Shader.TileMode.CLAMP)
        c.drawPath(body, p); p.shader = null

        // cheeks
        p.color = if (happy) 0x8CFFBEA5.toInt() else 0x66FFBEA5.toInt()
        rect.set(-25.2f, 4.6f, -14.8f, 11.4f); c.drawOval(rect, p)
        rect.set(14.8f, 4.6f, 25.2f, 11.4f); c.drawOval(rect, p)

        // eyes
        p.color = 0xFF2A1913.toInt()
        for (ex in floatArrayOf(-11f, 11f)) {
            if (happy) {
                p.style = Paint.Style.STROKE; p.strokeWidth = 3f
                rect.set(ex - 5.2f, -2f - 5.2f + 2f, ex + 5.2f, -2f + 5.2f + 2f)
                c.drawArc(rect, 200f, 140f, false, p); p.style = Paint.Style.FILL
            } else if (state == "error") {
                p.style = Paint.Style.STROKE; p.strokeWidth = 2.8f
                c.drawLine(ex - 4.5f, -8.5f, ex + 4.5f, 0.5f, p); c.drawLine(ex + 4.5f, -8.5f, ex - 4.5f, 0.5f, p); p.style = Paint.Style.FILL
            } else {
                var ry = if (state == "listening") 7.4f else if (state == "thinking") 5f else 6.3f
                if (state == "idle" && t % 3800 > 3660) ry *= 0.15f
                rect.set(ex - 4.6f, -4f - max(0.6f, ry), ex + 4.6f, -4f + max(0.6f, ry)); c.drawOval(rect, p)
            }
        }
        // mouth
        p.style = Paint.Style.STROKE; p.strokeWidth = 2.6f
        body.reset()
        when {
            state == "listening" -> { p.style = Paint.Style.FILL; val o = 1.5f + talk * 7.5f; rect.set(-3.6f - talk * 1.6f, 11f - o, 3.6f + talk * 1.6f, 11f + o); c.drawOval(rect, p) }
            happy -> { body.moveTo(-7f, 8f); body.quadTo(0f, 18f, 7f, 8f); c.drawPath(body, p) }
            state == "thinking" -> c.drawLine(-4f, 12f, 4f, 12f, p)
            state == "error" -> { body.moveTo(-6f, 15f); body.quadTo(0f, 8f, 6f, 15f); c.drawPath(body, p) }
            else -> { body.moveTo(-5f, 10f); body.quadTo(0f, 14.5f, 5f, 10f); c.drawPath(body, p) }
        }
        c.restore()
        postInvalidateOnAnimation()   // idle blink and breathing also animate; cheap for one small view
    }
}
