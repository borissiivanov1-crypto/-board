package app.board

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LogicTest {
    private val sensors = JSONArray("""[{"label":"Package id 0","value":71,"unit":"C","type":"temperature_core"},{"label":"Core 0","value":66,"unit":"C","type":"temperature_core"},{"label":"Core 1","value":88,"unit":"C","type":"temperature_core"},
        {"label":"Composite","value":52,"unit":"C","type":"temperature_core"},{"label":"/dev/sda","value":41,"unit":"C","type":"temperature_hdd"},{"label":"/dev/sdb","value":57,"unit":"C","type":"temperature_hdd"}]""")
    private val fs = JSONArray("""[{"mnt_point":"/","device_name":"/dev/nvme0n1p2","fs_type":"ext4","size":500e9,"used":210e9,"free":290e9,"percent":42},
        {"mnt_point":"/DATA","device_name":"/dev/md0","fs_type":"ext4","size":8e12,"used":6.9e12,"free":1.1e12,"percent":86.2},
        {"mnt_point":"/var/lib/docker","device_name":"overlay","fs_type":"overlay","size":1,"used":1,"percent":1}]""")

    @Test fun temps() {
        val t = parseTemps(sensors)
        assertEquals(71.0, t.pkg)
        assertEquals(listOf(66.0, 88.0), t.cores)
        assertEquals(1, t.nvme.size)
        assertEquals(2, t.hdd.size)
        assertEquals(71.0 to 88.0, cpuTemp(t))
        val disks = Glances(Settings()).pickDisks(fs)
        assertEquals(listOf("DATA", "Root"), disks.map { it.name })
        assertEquals(DriveTemp("hdd", listOf(41.0, 57.0)), driveTemp(disks[0], t))
        assertEquals(DriveTemp("nvme", listOf(52.0)), driveTemp(disks[1], t))
        assertEquals(2, sumStorage(disks)!!.count)
    }

    @Test fun settings() {
        val s = cleanSettings(JSONObject("""{"serverHost":"https://nas.lan:8443/x","platform":"proxmox","liveSeconds":"99","tempLimits":{"cpu":{"warm":80,"hot":70}}}"""))
        assertEquals("nas.lan", s.serverHost); assertEquals("https", s.serverScheme); assertEquals("8443", s.serverPort)
        assertEquals(30, s.liveSeconds)
        assertEquals(TempLimit(80, 81), s.tempLimits["cpu"])
        assertEquals("https://nas.lan:8443", serverUrlOf(s))
        assertEquals("http://nas.lan:61208", glancesUrl(s))
        // older ZimaOS-only settings (showPower, no platform) stay on ZimaOS
        assertEquals("zimaos", cleanSettings(JSONObject("""{"showPower":true}""")).platform)
        assertEquals("none", cleanSettings(JSONObject()).platform)
        assertEquals("https://10.0.0.5:8006", serverUrlOf(cleanSettings(JSONObject("""{"serverHost":"10.0.0.5","platform":"proxmox"}"""))))
        assertNull(cleanSettings(JSONObject("""{"baseUrl":"nope"}""")).baseUrl.ifEmpty { null })
    }

    @Test fun bytes() {
        assertEquals("1.0 KB", formatBytes(1024.0)); assertEquals("512 B", formatBytes(512.0)); assertEquals("196 GB", formatBytes(210e9))
    }
}
