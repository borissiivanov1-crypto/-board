package app.board

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ZimaTest {
    private fun svc(url: String, port: String = "", zima: String = "") =
        Service("c:1", "X", port, "🔗", "", url, true, "http", "", "", true, zima = zima)

    private val apps = listOf(
        ZApp("jellyfin", "Jellyfin", "", "8096", "http", "", "running", "", "v2app", "jellyfin"),
        ZApp("immich", "Immich", "", "2283", "http", "", "exited", "", "v2app", "immich"),
    )

    @Test fun parsesGridItem() {
        val a = Zima.parseApp(JSONObject("""{"name":"jellyfin","title":{"en_us":"Jellyfin"},"port":8096,"scheme":"http","index":"web","status":"running","app_type":"v2app","icon":"https://x/j.png"}"""))!!
        assertEquals("Jellyfin", a.title)
        assertEquals("8096", a.port)
        assertEquals("/web", a.path)
        assertEquals(true, a.running)
        assertEquals(false, Zima.parseApp(JSONObject("""{"name":"x","app_type":"link"}"""))!!.controllable)
    }

    @Test fun linksByPortOrId() {
        assertEquals("jellyfin", Zima.appFor(svc("http://192.168.1.10:8096"), apps, "192.168.1.10")?.id)
        assertNull(Zima.appFor(svc("http://other.host:8096"), apps, "192.168.1.10"))
        assertNull(Zima.appFor(svc("http://192.168.1.10:9999"), apps, "192.168.1.10"))
        assertEquals("immich", Zima.appFor(svc("https://photos.example.com", zima = "immich"), apps, "192.168.1.10")?.id)
        // two apps on one port: not linked; no port = the usual one for http / https
        val two = apps + ZApp("other", "Other", "", "8096", "http", "", "running", "", "v2app", "other")
        assertNull(Zima.appFor(svc("http://192.168.1.10:8096"), two, "192.168.1.10"))
        val web = listOf(ZApp("web", "Web", "", "80", "http", "", "running", "", "v2app", "web"))
        assertEquals("web", Zima.appFor(svc("http://192.168.1.10"), web, "192.168.1.10")?.id)
        assertEquals("http://192.168.1.10/web", Zima.appUrl(web[0].copy(path = "/web"), "192.168.1.10"))
    }

    @Test fun controllableAndRunning() {
        val a = Zima.parseApp(JSONObject("""{"name":"x","app_type":"v2app","status":"Up (healthy)","is_uncontrolled":true}"""))!!
        assertEquals(true, a.running)
        assertEquals(false, a.controllable)
    }

    @Test fun matchesMountsToDrives() {
        val drives = listOf(ZDrive("sda", "", "/dev/sda", "HDD", true, 35.0), ZDrive("nvme0n1", "", "/dev/nvme0n1", "SSD", true, 41.0))
        assertEquals("sda", Zima.driveFor("/dev/sda1", drives)?.name)
        assertEquals("nvme0n1", Zima.driveFor("/dev/nvme0n1p2", drives)?.name)
        assertNull(Zima.driveFor("/dev/sdb1", drives))
    }
}
