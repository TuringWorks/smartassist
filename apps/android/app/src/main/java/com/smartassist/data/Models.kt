package com.smartassist.data

import java.util.Date
import java.util.UUID

data class Device(
    val id: String,
    val name: String,
    val deviceType: String,
    val pairedAt: Date? = null,
    val lastSeen: Date? = null,
    val connected: Boolean = false
)

data class ChatMessage(
    val id: String = UUID.randomUUID().toString(),
    val role: String,
    val content: String,
    val timestamp: Date = Date()
)

data class Session(
    val id: String = UUID.randomUUID().toString(),
    val status: String = "active",
    val messages: MutableList<ChatMessage> = mutableListOf(),
    val createdAt: Date = Date()
)
