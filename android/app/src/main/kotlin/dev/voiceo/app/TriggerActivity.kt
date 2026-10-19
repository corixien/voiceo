package dev.voiceo.app

import android.app.Activity
import android.os.Bundle

/**
 * Invisible trampoline for launcher shortcuts and the side-key "open app" action:
 * toggles dictation and finishes at once, so focus returns to the app the user was typing in.
 */
class TriggerActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Dictation.toggle(this)
        finish()
        overridePendingTransition(0, 0)
    }
}
