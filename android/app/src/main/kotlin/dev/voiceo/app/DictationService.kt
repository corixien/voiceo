package dev.voiceo.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.os.IBinder
import android.os.VibrationEffect
import android.os.Vibrator
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/** Foreground service (microphone): records, runs the pipeline, hands the text to the accessibility service. */
class DictationService : Service() {
    private lateinit var store: Store
    private lateinit var groq: Groq
    @Volatile private var recording = false
    private var thread: Thread? = null
    private var buf = FloatArray(Dsp.RATE * 30)
    private var len = 0
    private var startedAt = 0L

    override fun onBind(i: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        store = Store(this)
        groq = Groq(this) { msg -> Dictation.emit("thinking", msg) }
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Diktat", NotificationManager.IMPORTANCE_LOW))
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> startRecording()
            ACTION_STOP -> stopRecording()
            ACTION_RETRY -> retryFailed()
            else -> stopSelf()
        }
        return START_NOT_STICKY
    }

    private fun notification(text: String): Notification {
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(this, 1, Intent(this, DictationService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE)
        return NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_mic)
            .setContentTitle("Voiceo")
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .setSilent(true)
            .addAction(0, "Stopp", stop)
            .build()
    }

    private fun goForeground(text: String): Boolean = try {
        ServiceCompat.startForeground(this, 1, notification(text), ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        true
    } catch (e: Exception) {
        Dictation.emit("error", "Mikrofon nicht verfügbar: ${e.message}")
        Dictation.postDelayed(3500) { Dictation.emit("idle") }
        stopSelf()
        false
    }

    private fun vibrate(ms: Long) {
        val v = getSystemService(Vibrator::class.java) ?: return
        v.vibrate(VibrationEffect.createOneShot(ms, VibrationEffect.DEFAULT_AMPLITUDE))
    }

    private fun startRecording() {
        if (recording) return
        if (!goForeground("Hört zu …")) return
        val minBuf = AudioRecord.getMinBufferSize(Dsp.RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val rec = try {
            AudioRecord(MediaRecorder.AudioSource.MIC, Dsp.RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, maxOf(minBuf, Dsp.RATE / 2))
        } catch (e: SecurityException) {
            Dictation.emit("error", "Mikrofon-Berechtigung fehlt"); stopSelf(); return
        }
        if (rec.state != AudioRecord.STATE_INITIALIZED) {
            rec.release(); Dictation.emit("error", "Mikrofon nicht verfügbar")
            Dictation.postDelayed(3500) { Dictation.emit("idle") }; stopSelf(); return
        }
        len = 0; startedAt = System.currentTimeMillis(); recording = true
        Dictation.emit("listening")
        vibrate(25)
        val maxSamples = store.settings().optInt("max_seconds", 300).coerceAtLeast(5) * Dsp.RATE
        thread = Thread({
            val chunk = ShortArray(Dsp.RATE / 50)
            var lastEmit = 0L
            try {
                rec.startRecording()
                while (recording) {
                    val n = rec.read(chunk, 0, chunk.size)
                    if (n <= 0) { if (n < 0) break else continue }
                    if (len + n > buf.size) buf = buf.copyOf(buf.size * 2)
                    var sq = 0f
                    for (i in 0 until n) { val f = chunk[i] / 32768f; buf[len + i] = f; sq += f * f }
                    len += n
                    val now = System.currentTimeMillis()
                    if (now - lastEmit >= 20) {
                        lastEmit = now
                        Dictation.emitAudio(kotlin.math.sqrt(sq / n), Dsp.bands(buf.copyOfRange(maxOf(0, len - 1024), len)))
                    }
                    if (len >= maxSamples) { Dictation.post { stopRecording() }; break }
                }
            } finally {
                runCatching { rec.stop() }
                rec.release()
            }
        }, "voiceo-record").also { it.start() }
    }

    private fun stopRecording() {
        if (!recording) { if (Dictation.state == "idle") stopSelf(); return }
        recording = false
        vibrate(15)
        val t0 = startedAt
        Thread {
            thread?.join(1500)
            val samples = buf.copyOf(len)
            if (System.currentTimeMillis() - t0 < MIN_RECORDING_MS || samples.size < Dsp.RATE / 3) {
                Dictation.emit("idle"); finish(); return@Thread
            }
            process(samples, t0)
        }.start()
    }

    private fun retryFailed() {
        val samples = Dictation.failedSamples
        if (samples == null || Dictation.state != "idle") { stopSelf(); return }
        Dictation.failedSamples = null
        if (!goForeground("Verarbeite …")) return
        Thread { process(samples, System.currentTimeMillis()) }.start()
    }

    private fun process(samples: FloatArray, t0: Long) {
        Dictation.emit("thinking")
        try {
            val out = Pipeline.run(this, store, groq, samples)
            if (out == null) { Dictation.emit("idle"); return }
            val s = store.settings()
            store.addHistory(out.raw, out.text, s.optString("mode"), System.currentTimeMillis() - t0, s.optInt("history_limit", 100))
            val warn = deliver(out.text, s)
            if (warn != null) { Dictation.emit("error", warn); Dictation.postDelayed(3500) { Dictation.emit("idle") } }
            else if (out.warn != null) { Dictation.emit("error", out.warn); Dictation.postDelayed(3500) { Dictation.emit("idle") } }
            else { Dictation.emit("done", out.text); Dictation.postDelayed(1100) { Dictation.emit("idle") } }
        } catch (e: Groq.ApiError) {
            val msg = e.message ?: "Fehler"
            if (Groq.isTransient(msg)) {
                Dictation.failedSamples = samples
                notifyRetry()
                Dictation.emit("error", "Groq nicht erreichbar. Benachrichtigung: Wiederholen")
            } else Dictation.emit("error", msg)
            Dictation.postDelayed(4500) { Dictation.emit("idle") }
        } catch (e: Exception) {
            Dictation.emit("error", e.message ?: e.javaClass.simpleName)
            Dictation.postDelayed(4500) { Dictation.emit("idle") }
        } finally {
            finish()
        }
    }

    /** Inserts via the accessibility service; falls back to the clipboard. Returns a warning or null. */
    private fun deliver(text: String, s: org.json.JSONObject): String? {
        if (!s.optBoolean("auto_insert", true)) { copy(text); return "In Zwischenablage kopiert" }
        val svc = VoiceoAccessibilityService.instance
        if (svc == null) { copy(text); return "Bedienungshilfe aus: Text in Zwischenablage" }
        val done = java.util.concurrent.CompletableFuture<Boolean>()
        Dictation.post { done.complete(svc.insertText(text, s)) }
        val ok = runCatching { done.get(3, java.util.concurrent.TimeUnit.SECONDS) }.getOrDefault(false)
        if (!ok) { copy(text); return "Kein Textfeld aktiv: in Zwischenablage kopiert" }
        return null
    }

    private fun copy(text: String) {
        Dictation.post {
            val cm = getSystemService(ClipboardManager::class.java)
            cm.setPrimaryClip(ClipData.newPlainText("Voiceo", text))
        }
    }

    private fun notifyRetry() {
        val retry = PendingIntent.getForegroundService(this, 2, Intent(this, DictationService::class.java).setAction(ACTION_RETRY), PendingIntent.FLAG_IMMUTABLE)
        val n = NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_mic).setContentTitle("Voiceo")
            .setContentText("Diktat nicht verarbeitet. Erneut versuchen?")
            .addAction(0, "Wiederholen", retry).setAutoCancel(true).build()
        getSystemService(NotificationManager::class.java).notify(2, n)
    }

    private fun finish() {
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        recording = false
        super.onDestroy()
    }

    companion object {
        const val ACTION_START = "dev.voiceo.app.START"
        const val ACTION_STOP = "dev.voiceo.app.STOP"
        const val ACTION_RETRY = "dev.voiceo.app.RETRY"
        const val CHANNEL = "dictation"
        const val MIN_RECORDING_MS = 300L
    }
}
