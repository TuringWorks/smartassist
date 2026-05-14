Feature: Gateway health and boot

  Scenario: Gateway reports healthy status
    Given the SmartAssist gateway is running
    When I request gateway health
    Then the response status should be 200
    And the response should contain 'ok'

  Scenario: Gateway boots with default configuration
    Given the SmartAssist gateway is running
    When I request gateway health
    Then the response status should be 200
    And the response should contain 'status'
