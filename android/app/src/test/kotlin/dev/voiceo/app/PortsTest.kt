package dev.voiceo.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.PI
import kotlin.math.sin

class PortsTest {
    @Test fun learnsAndApplies() {
        val p = Learn.diffPairs("Bitte schreibe dies in clod.ai und danke", "Bitte schreibe dies in Claude.ai und danke")
        assertEquals(listOf("clod.ai" to "Claude.ai"), p)
        val items = listOf(Learned("clod.ai", "Claude.ai", 1))
        assertEquals("Gehe zu Claude.ai, bitte", Learn.apply("Gehe zu clod.ai, bitte", items))
    }

    @Test fun addMergesCounts() {
        val l = mutableListOf<Learned>()
        Learn.add(l, "clod", "Claude"); Learn.add(l, "clod", "Claude"); Learn.add(l, "clod", "Claudius")
        assertEquals(listOf(Learned("clod", "Claudius", 3)), l)
        Learn.add(l, "", "Kubernetes")
        assertEquals(2, l.size)
    }

    private fun burst(seconds: Float, from: Float, to: Float, total: Float): FloatArray {
        val n = (total * Dsp.RATE).toInt()
        val noise = java.util.Random(1)
        return FloatArray(n) { i ->
            val t = i / Dsp.RATE.toFloat()
            val bg = (noise.nextGaussian() * 0.001).toFloat()
            if (t in from..to) (0.3f * sin(2 * PI * 220 * t * (1 + 0.3 * sin(2 * PI * 3 * t)))).toFloat() + bg else bg
        }
    }

    @Test fun vadTrimsSilenceAndKeepsSpeech() {
        val x = burst(1f, 2f, 3f, 5f)
        val out = Dsp.vadSoft(x)
        assertNotNull(out)
        assertTrue("shorter than input", out!!.size < x.size)
        assertTrue("keeps speech plus padding", out.size > Dsp.RATE)
    }

    @Test fun vadRejectsSilence() {
        assertNull(Dsp.vadSoft(FloatArray(Dsp.RATE * 2) { 0.0005f * sin(it * 0.01f) }))
    }

    @Test fun wavHeader() {
        val w = Dsp.toWav(FloatArray(100))
        assertEquals(44 + 200, w.size)
        assertEquals("RIFF", String(w, 0, 4)); assertEquals("WAVE", String(w, 8, 4))
    }

    @Test fun promptEchoGuard() {
        assertTrue(Groq.isPromptEcho("Ja, also ich hab das gestern mal ausprobiert, und es funktioniert eigentlich ganz gut."))
        assertTrue(!Groq.isPromptEcho("Wir müssen das Meeting verschieben, weil der Client ein neues Requirement geschickt hat."))
    }

    @Test fun bandsOfToneConcentrate() {
        val x = FloatArray(1024) { sin(2 * PI * 1000 * it / Dsp.RATE).toFloat() * 0.5f }
        val b = Dsp.bands(x)
        val peak = b.indices.maxByOrNull { b[it] }!!
        assertTrue("1 kHz lands in the middle bands, got $peak", peak in 6..10)
    }
}
