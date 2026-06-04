package dev.suzaku.android.ime

import android.os.Bundle
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

class SuzakuPanelActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_panel)

        findViewById<TextView>(R.id.bootstrapStatus).text =
            runCatching { SuzakuNativeBridge.nativeDescribeBootstrap() }
                .getOrElse { "Android bootstrap unavailable" }

        findViewById<TextView>(R.id.panelStatus).text =
            runCatching { SuzakuNativeBridge.nativeDescribePanelCompanion() }
                .getOrElse { "Panel companion unavailable" }
    }
}
