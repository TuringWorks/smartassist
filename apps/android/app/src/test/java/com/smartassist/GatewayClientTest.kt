package com.smartassist

import com.smartassist.network.GatewayClient
import org.junit.Test
import org.junit.Assert.*

class GatewayClientTest {
    @Test
    fun testDefaultBaseUrl() {
        val client = GatewayClient()
        assertEquals("http://localhost:18789", client.baseUrl)
    }

    @Test
    fun testSetAuthToken() {
        val client = GatewayClient()
        client.setAuthToken("test-token")
        // The token is private, but we can verify no exception is thrown
    }
}
