package app.board

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class FailoverTest {
    private fun settings(vararg kv: Pair<String, Any>) = cleanSettings(JSONObject().apply { for ((k, v) in kv) put(k, v) })

    @Test fun cleansFailoverAddress() {
        val s = settings("serverHost" to "100.64.0.5", "failover" to true, "failoverHost" to "http://192.168.1.10:8080/x")
        assertEquals("192.168.1.10", s.failoverHost)
        assertTrue(failoverReady(s))
        val json = s.toJson()
        assertEquals(true, json.get("failover"))
        assertEquals("192.168.1.10", json.get("failoverHost"))
    }

    @Test fun offOrEmptyOrSameIsNotReady() {
        assertFalse(failoverReady(settings("serverHost" to "100.64.0.5", "failover" to false, "failoverHost" to "192.168.1.10")))
        assertFalse(failoverReady(settings("serverHost" to "100.64.0.5", "failover" to true)))
        assertFalse(failoverReady(settings("serverHost" to "192.168.1.10", "failover" to true, "failoverHost" to "192.168.1.10")))
        assertFalse(failoverReady(Settings()))
    }

    @Test fun swapsOnlyTheServerHost() {
        assertEquals("http://192.168.1.10:8096/web", swapHost("http://100.64.0.5:8096/web", "100.64.0.5", "192.168.1.10"))
        assertEquals("https://app.example.com", swapHost("https://app.example.com", "100.64.0.5", "192.168.1.10"))
    }

    @Test fun viaFailoverMovesServerGlancesAndPowerAddresses() {
        val s = settings("serverHost" to "100.64.0.5", "serverPort" to "90", "failover" to true, "failoverHost" to "192.168.1.10",
            "baseUrl" to "http://100.64.0.5:61208", "platform" to "proxmox", "proxmoxUrl" to "https://other.host:8006")
        val f = viaFailover(s)
        assertEquals("192.168.1.10", serverHostOf(f))
        assertEquals("https://192.168.1.10:90", serverUrlOf(f))
        assertEquals("http://192.168.1.10:61208", glancesUrl(f))
        assertEquals("https://other.host:8006", f.url("proxmoxUrl"))
    }

    @Test fun routeIsPerServer() {
        val s = settings("serverHost" to "100.64.0.5", "failover" to true, "failoverHost" to "192.168.1.10")
        Route.set("a", true)
        assertEquals("192.168.1.10", serverHostOf(effective(s, "a")))
        assertEquals("100.64.0.5", serverHostOf(effective(s, "b")))
        Route.set("a", false)
        assertEquals("100.64.0.5", serverHostOf(effective(s, "a")))
        Route.forget("a")
    }
}
