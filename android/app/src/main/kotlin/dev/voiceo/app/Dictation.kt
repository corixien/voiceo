package dev.voiceo.app

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import androidx.core.content.ContextCompat

/** Process-wide dictation state. UI pieces (overlay, tile, settings) subscribe via [listeners]. */
object Dictation {
    interface Listener {
        fun onState(state: String, message: String?) {}
        fun onAudio(rms: Float, bands: FloatArray) {}
    }

    @Volatile var state = "idle"        // idle | listening | thinking
    @Volatile var failedSamples: FloatArray? = null
    val listeners = java.util.concurrent.CopyOnWriteArrayList<Listener>()
    private val main = Handler(Looper.getMainLooper())

    fun post(r: Runnable) { main.post(r) }
    fun postDelayed(ms: Long, r: Runnable) { main.postDelayed(r, ms) }

    /** state: idle | listening | thinking | done | learned | error */
    fun emit(state: String, message: String? = null) {
        if (state == "listening" || state == "thinking" || state == "idle") this.state = state
        main.post { listeners.forEach { it.onState(state, message) } }
    }

    fun emitAudio(rms: Float, bands: FloatArray) { main.post { listeners.forEach { it.onAudio(rms, bands) } } }

    fun hasMic(ctx: Context) = ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    fun toggle(ctx: Context) {
        when (state) {
            "listening" -> stop(ctx)
            "idle" -> start(ctx)
            else -> {}
        }
    }

    fun start(ctx: Context) {
        if (!hasMic(ctx)) {
            Toast.makeText(ctx, "Voiceo: Mikrofon-Berechtigung fehlt", Toast.LENGTH_LONG).show()
            ctx.startActivity(Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            return
        }
        ContextCompat.startForegroundService(ctx, Intent(ctx, DictationService::class.java).setAction(DictationService.ACTION_START))
    }

    fun stop(ctx: Context) {
        ctx.startService(Intent(ctx, DictationService::class.java).setAction(DictationService.ACTION_STOP))
    }

    fun retry(ctx: Context) {
        ContextCompat.startForegroundService(ctx, Intent(ctx, DictationService::class.java).setAction(DictationService.ACTION_RETRY))
    }
}
