package dev.voiceo.app

import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/** Quick Settings tile: tap to start, tap again to stop and insert. */
class DictateTileService : TileService(), Dictation.Listener {
    override fun onStartListening() {
        Dictation.listeners.add(this)
        refresh()
    }

    override fun onStopListening() {
        Dictation.listeners.remove(this)
    }

    override fun onClick() {
        Dictation.toggle(this)
    }

    override fun onState(state: String, message: String?) = refresh()

    private fun refresh() {
        val t = qsTile ?: return
        t.state = if (Dictation.state == "idle") Tile.STATE_INACTIVE else Tile.STATE_ACTIVE
        t.label = "Voiceo"
        t.subtitle = when (Dictation.state) { "listening" -> "Hört zu …"; "thinking" -> "Verarbeite …"; else -> "Diktat" }
        t.updateTile()
    }
}
