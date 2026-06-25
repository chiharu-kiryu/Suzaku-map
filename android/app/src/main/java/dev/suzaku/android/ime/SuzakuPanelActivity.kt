package dev.suzaku.android.ime

import android.os.Bundle
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

class SuzakuPanelActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_panel)

        findViewById<TextView>(R.id.bootstrapStatus).text =
            runCatching {
                buildString {
                    append(SuzakuNativeBridge.nativeDescribeImeHost())
                    append("\n\n")
                    append(SuzakuNativeBridge.nativeDescribeBootstrap())
                }
            }.getOrElse { "Android bootstrap unavailable" }

        findViewById<TextView>(R.id.panelStatus).text =
            runCatching {
                buildString {
                    append(SuzakuNativeBridge.nativeRegistrationHint())
                    append("\n")
                    append(SuzakuNativeBridge.nativeDescribeImeDispatch())
                    append("\n")
                    append(SuzakuNativeBridge.nativeDescribePanelCompanion())
                }
            }.getOrElse { "Panel companion unavailable" }
    }
}
