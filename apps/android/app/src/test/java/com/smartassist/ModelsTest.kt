package com.smartassist

import com.smartassist.data.ChatMessage
import com.smartassist.data.Device
import com.smartassist.data.Session
import org.junit.Test
import org.junit.Assert.*

class ModelsTest {
    @Test
    fun testDeviceInitialization() {
        val device = Device(id = "test-1", name = "Test Device", deviceType = "android")
        assertEquals("test-1", device.id)
        assertEquals("Test Device", device.name)
        assertEquals("android", device.deviceType)
        assertFalse(device.connected)
    }

    @Test
    fun testChatMessageInitialization() {
        val message = ChatMessage(role = "user", content = "Hello")
        assertEquals("user", message.role)
        assertEquals("Hello", message.content)
    }

    @Test
    fun testSessionInitialization() {
        val session = Session()
        assertEquals("active", session.status)
        assertTrue(session.messages.isEmpty())
    }
}
