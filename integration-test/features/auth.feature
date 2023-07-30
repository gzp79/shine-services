Feature: Auth Status

  Scenario: Get provider
    Given a GET request to /api/auth/provider
     When the response is received
     Then the response should have a status 200
