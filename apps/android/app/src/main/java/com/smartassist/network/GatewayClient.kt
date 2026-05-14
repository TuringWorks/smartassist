package com.smartassist.network

import io.ktor.client.HttpClient
import io.ktor.client.engine.android.Android
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.client.plugins.websocket.WebSockets
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsText
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.serialization.kotlinx.json.json
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject

class GatewayClient {
    var baseUrl: String = "http://localhost:18789"
    private var authToken: String? = null

    private val client = HttpClient(Android) {
        install(ContentNegotiation) {
            json(Json { ignoreUnknownKeys = true })
        }
        install(WebSockets)
    }

    fun setAuthToken(token: String) {
        authToken = token
    }

    suspend fun sendRPC(method: String, params: Map<String, String>? = null): JsonObject =
        withContext(Dispatchers.IO) {
            val body = JsonObject(
                mapOf(
                    "jsonrpc" to JsonPrimitive("2.0"),
                    "id" to JsonPrimitive(System.currentTimeMillis().toString()),
                    "method" to JsonPrimitive(method),
                    "params" to JsonObject(params ?: emptyMap())
                )
            )

            val response = client.post("$baseUrl/rpc") {
                contentType(ContentType.Application.Json)
                authToken?.let {
                    headers["Authorization"] = "Bearer $it"
                }
                setBody(body)
            }

            val text = response.bodyAsText()
            Json.parseToJsonElement(text).jsonObject
        }

    suspend fun pairDevice(deviceId: String, name: String): String =
        withContext(Dispatchers.IO) {
            val result = sendRPC(
                "device.pair",
                mapOf(
                    "device_id" to deviceId,
                    "name" to name,
                    "device_type" to "android"
                )
            )
            val response = result["result"]?.jsonObject
                ?: throw IllegalStateException("Pairing failed")
            response["challenge_code"]?.toString()?.trim('"')
                ?: throw IllegalStateException("No challenge code")
        }

    suspend fun listDevices(): List<com.smartassist.data.Device> =
        withContext(Dispatchers.IO) {
            val result = sendRPC("device.pair.list")
            val response = result["result"]?.jsonObject
            val devices = response?.get("devices")?.toString() ?: "[]"
            // Simplified parsing - in a real app use kotlinx.serialization
            emptyList()
        }
}
