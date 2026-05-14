Feature: End-to-End Smoke Test
  A full-stack scenario that exercises the core SmartAssist pipeline
  from gateway boot through agent execution and channel response.

  Scenario: Full stack smoke test
    Given the SmartAssist gateway is configured with default settings
    When the gateway boots
    Then the gateway health endpoint should return status 200

    Given an agent "smoke-agent" is configured with model "anthropic/claude-sonnet-4-5"
    When the agent is spawned
    Then the agent status should be "online"

    Given a mock channel "test-web" is registered
    When the channel connects
    Then the channel health should be "healthy"

    When a user sends "Hello" via channel "test-web"
    Then the gateway should route the message to agent "smoke-agent"
    And the agent should process the message
    And a response should be queued for delivery
    And the delivery queue should have 1 pending message

  Scenario: Gateway boot and RPC surface
    Given the gateway is configured on port 18789
    When the gateway boots
    Then the RPC method "system.info" should be registered
    And the RPC method "ping" should be registered
    And the RPC method "agent.run" should be registered
    And the RPC method "channel.list" should be registered

  Scenario: Agent tool execution roundtrip
    Given agent "tool-agent" is configured with the file-system tool group
    When the agent receives "read file /tmp/hello.txt"
    Then the agent should invoke the "read_file" tool
    And the tool result should be returned to the agent
    And the agent should produce a final response containing the tool result

  Scenario: Channel auto-reply integration
    Given a mock channel "telegram-bot" is registered
    And an auto-reply rule "greet" exact "/start" replies "Welcome!" is active
    When a message "/start" arrives from "telegram-bot"
    Then the auto-reply engine should generate "Welcome!"
    And the outbound message should target the same chat

  Scenario: Security audit baseline
    Given the security module is loaded
    When the exec surface audit runs
    Then it should complete without critical errors

  Scenario: Doctor reports all features healthy
    Given SmartAssist is fully configured
    When the doctor command runs with --full
    Then the output should contain "16/16 healthy"
    And the error count should be 0
