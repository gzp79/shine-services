    @ignore
Feature: OpenId Connect mocker server

  Background:
    * def base = 'http://mock.localhost.com:8090'

  Scenario: pathMatches('/openid/.well-known/openid-configuration') && methodIs('get')
    * def response = 
    """
    {
        'issuer': "#(base + '/openid')",
        'authorize_endpoint': "#(base + '/openid/authorize')",
        'token_endpoint': "#(base + '/openid/token')",
        'userinfo_endpoint': "#(base + '/openid/userinfo')",
    }
    """

  Scenario: pathMatches('/stop') && methodIs('get')
    * print 'Stopping karate...'
    * eval java.lang.System.exit(0)
