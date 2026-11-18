package app.board

import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import app.board.ui.BoardApp

class MainActivity : ComponentActivity() {
    private val vm: BoardVM by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(SystemBarStyle.dark(Color.TRANSPARENT), SystemBarStyle.dark(Color.TRANSPARENT))
        super.onCreate(savedInstanceState)
        if (savedInstanceState == null) handle(intent)
        setContent { BoardApp(vm) }
        BoardWidget.schedule(this, vm.set.refreshMinutes)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    // opened from the widget: show that server (and that stack)
    private fun handle(i: Intent?) {
        val server = i?.getStringExtra("server")
        if (!server.isNullOrEmpty() && Store.hasServer(server) && server != Store.CUR) {
            vm.switchServer(server, 0)
        }
        i?.getStringExtra("stack")?.let { vm.focusStackReq = it }
    }

    override fun onResume() {
        super.onResume()
        vm.onResume()
    }

    override fun onPause() {
        super.onPause()
        vm.onPause()
    }
}
