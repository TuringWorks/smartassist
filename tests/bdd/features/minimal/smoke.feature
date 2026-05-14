Feature: Minimal smoke test

  Scenario: Gateway is running
    Given the SmartAssist gateway is running
    When I request gateway health
    Then the response status should be 200
