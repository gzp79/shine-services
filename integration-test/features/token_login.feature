Feature: Login

  Scenario: Login with token
    Given a "token" login request
    When the response is received
    Then the response should have a status 200
    And the response should have an body with "<html>"
    And the response should have an body with "<htmll>"
