
@ignore
Feature: Default for login

  Scenario: Common cokkie and login defaults
    * def SESSION_SCOPE = -9223372036854775808
    * def redirects = 
    """ 
    {
        loginUrl: 'http://login.com/',
        redirectUrl: 'http://redirect.com/',
        errorUrl: 'http://error.com/'
    } 
    """
    # common properties of the cookies
    * def cookieDefaults = 
    """ 
    {
        tid: {path: '/identity/auth', domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #notnull, samesite:'Lax'},
        sid: {path: '/', domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #notnull, samesite:'Lax'},
        eid: {path: '/identity/auth', domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #notnull, samesite:'Lax'},
    } 
    """
    # cookie values to remove them from the client
    * def cookieNone = 
    """ 
    {
        tid: {path: '/identity/auth', domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #notnull, samesite:'Lax', 'max-age':#? _ < 0},
        sid: {path: '/', domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #notnull, samesite:'Lax', 'max-age':#? _ < 0},
        eid: {path: '/identity/auth', domain:#(karate.properties['serviceDomain']), httponly:true, secure: true, value: #notnull, samesite:'Lax', 'max-age':#? _ < 0},
    } 
    """    
    