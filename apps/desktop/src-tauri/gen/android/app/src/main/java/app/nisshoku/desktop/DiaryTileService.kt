package app.nisshoku.desktop

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService

/** A Quick Settings tile that opens today's diary entry. */
class DiaryTileService : TileService() {
  override fun onStartListening() {
    super.onStartListening()
    val tile = qsTile ?: return
    tile.state = Tile.STATE_INACTIVE
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = getString(R.string.tile_diary_subtitle)
    tile.updateTile()
  }

  @SuppressLint("StartActivityAndCollapseDeprecated")
  override fun onClick() {
    super.onClick()
    val intent = Intent(this, MainActivity::class.java)
      .setAction(Intent.ACTION_VIEW)
      .setData(Uri.parse("nisshoku://diary"))
      .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    val open = {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        // Android 14 and later only take a PendingIntent.
        startActivityAndCollapse(
          PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        )
      } else {
        @Suppress("DEPRECATION")
        startActivityAndCollapse(intent)
      }
    }
    // On the lock screen, ask for the unlock first.
    if (isLocked) unlockAndRun(open) else open()
  }
}
