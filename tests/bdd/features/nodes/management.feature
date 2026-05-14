Feature: Node management

  Scenario: List empty nodes
    Given the SmartAssist gateway is running
    When I list all nodes
    Then the response status should be 200
    And the node count should be 0

  Scenario: Approve and list a node
    Given the SmartAssist gateway is running
    When I approve node pairing for 'node-1' with code '123456'
    And I list all nodes
    Then the node count should be 1
    And the response should contain 'node-1'

  Scenario: Describe a node
    Given the SmartAssist gateway is running
    And a node named 'node-alpha' is approved
    When I describe node 'node-alpha'
    Then the response status should be 200
    And the response should contain 'node-alpha'

  Scenario: Rename a node
    Given the SmartAssist gateway is running
    And a node named 'node-beta' is approved
    When I rename node 'node-beta' to 'node-beta-renamed'
    Then the response should contain 'renamed'

  Scenario: Unpair a node
    Given the SmartAssist gateway is running
    And a node named 'node-gamma' is approved
    When I unpair node 'node-gamma'
    Then the response should contain 'unpaired'
    When I list all nodes
    Then the node count should be 0
