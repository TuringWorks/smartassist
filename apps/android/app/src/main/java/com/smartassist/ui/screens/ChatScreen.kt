package com.smartassist.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.smartassist.data.ChatMessage
import com.smartassist.network.GatewayClient
import kotlinx.coroutines.launch

@Composable
fun ChatScreen(modifier: Modifier = Modifier) {
    val messages = remember { mutableStateListOf<ChatMessage>() }
    var input by remember { mutableStateOf("") }
    var isSending by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val gatewayClient = remember { GatewayClient() }

    Column(modifier = modifier.padding(16.dp)) {
        LazyColumn(
            modifier = Modifier.weight(1f),
            reverseLayout = true
        ) {
            items(messages.reversed()) { message ->
                MessageCard(message = message)
            }
        }

        error?.let {
            Text(
                text = it,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(vertical = 4.dp)
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OutlinedTextField(
                value = input,
                onValueChange = { input = it },
                label = { Text("Message") },
                modifier = Modifier.weight(1f),
                enabled = !isSending
            )

            TextButton(
                onClick = {
                    val text = input.trim()
                    if (text.isNotBlank()) {
                        messages.add(ChatMessage(role = "user", content = text))
                        input = ""
                        isSending = true
                        error = null

                        scope.launch {
                            try {
                                val response = gatewayClient.sendRPC(
                                    "chat",
                                    mapOf("message" to text, "agent_id" to "default")
                                )
                                val result = response["result"]
                                    ?.jsonObject
                                    ?.get("text")
                                    ?.toString()
                                    ?.trim('"')

                                if (result != null) {
                                    messages.add(ChatMessage(role = "assistant", content = result))
                                } else {
                                    error = "Unexpected response format"
                                }
                            } catch (e: Exception) {
                                error = "Failed to send: ${e.message}"
                            } finally {
                                isSending = false
                            }
                        }
                    }
                },
                enabled = input.isNotBlank() && !isSending
            ) {
                if (isSending) {
                    CircularProgressIndicator(modifier = Modifier.padding(4.dp))
                } else {
                    Text("Send")
                }
            }
        }
    }
}

@Composable
fun MessageCard(message: ChatMessage) {
    val isUser = message.role == "user"
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
    ) {
        Text(
            text = message.content,
            modifier = Modifier.padding(12.dp),
            color = if (isUser) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface
        )
    }
}
