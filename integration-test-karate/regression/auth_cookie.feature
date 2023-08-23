Feature: Check auth cookie validation

  Background:
    * def utils = call read("../utils/utils.js")  
    * url utils.identityUrl    
        
  Scenario Outline: tid: ${tid}, sid: ${sid}, eid: ${eid}
    * def data = (karate.callonce('@create_all_cookies').data)    
    * def getModifiedCookie = 
      """
        function(cookie, mod) {
          if( !'tse'.includes(cookie) ) 
            karate.abort()

          return mod == '-' ? null 
            : mod == '+' ? data[cookie] 
            : mod == '!' ? data[cookie.toUpperCase()]
            : mod == 's' ? 'invalid'.concat(data[cookie].slice(7))
            : karate.abort()
        }
      """
    * def getExpectedCookies = 
      """
      function(tokenString) {
          const r = {
            tid: {'max-age': '#? _ < 0'},
            sid: {'max-age': '#? _ < 0'},
            eid: {'max-age': '#? _ < 0'},
          };
          const tokens = (tokenString ?? '').split(',');
          for(const p of [['tid', ['t', 'T']], ['sid', ['s', 'S']], ['eid', ['e', 'E']]]) {
            for(const c of p[1]) {
              let rc = r[p[0]]
              if (tokens.includes(c)) {
                switch ( p[0] ) {
                  case 'tid': rc['max-age'] = '#? _ > 0'; break;
                  case 'sid': rc['max-age'] = utils.SESSION_SCOPE; break;
                  case 'eid': rc['max-age'] = utils.SESSION_SCOPE; break;
                }
                rc.value = data[c];
              }
            }
          }
          return r;
        }
      """
      
    Given path '/auth/validate'
      * configure cookies = 
      """
        ({
          tid: getModifiedCookie('t', tid),
          sid: getModifiedCookie('s', sid),
          eid: getModifiedCookie('e', eid),
        })
      """
      * method get
      * status 200
      * print note
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies contains deep getExpectedCookies(required)
    
    Examples:
      # -: missing
      # !: not matching (ex different user id). When multiple ! are present in a row it's assumed all of them are different
      # +: ok, valid cookie
      # s: signature missmatch
      | tid | sid | eid | required | note |
      |  s  |  -  |  -  |          | |     
      |  -  |  s  |  -  |          | |
      |  -  |  -  |  s  |          | |
      |  s  |  s  |  s  |          | |
      |  s  |  +  |  -  |  s       | It is equivalent as not providing a tid at all |

      |  -  |  -  |  -  |          | |
      |  -  |  -  |  +  |          | |
      |  -  |  -  |  !  |          | |
      |  -  |  +  |  -  |  s       | |
      |  -  |  +  |  +  |  s,e     | |
      |  -  |  +  |  !  |  s       | |     
      |  -  |  !  |  -  |  S       | |
      |  -  |  !  |  +  |  S       | |
      |  -  |  !  |  !  |  S       | |

      |  +  |  -  |  -  |  t       | |
      |  +  |  -  |  +  |  t       | |
      |  +  |  -  |  !  |  t       | |
      |  +  |  !  |  -  |  t       | |
      |  +  |  !  |  +  |  t       | |
      |  +  |  !  |  !  |  t       | |
      |  +  |  +  |  -  |  t,s     | |
      |  +  |  +  |  +  |  t,s,e   | |
      |  +  |  +  |  !  |  t,s     | |

      |  !  |  -  |  -  |  T       | |
      |  !  |  -  |  +  |  T       | |
      |  !  |  -  |  !  |  T       | |
      |  !  |  !  |  -  |  T       | |
      |  !  |  !  |  +  |  T       | |
      |  !  |  !  |  !  |  T       | |
      |  !  |  +  |  -  |  T       | |
      |  !  |  +  |  +  |  T       | |
      |  !  |  +  |  !  |  T       | |

    @ignore @create_all_cookies
  Scenario: Create all the required cookies
    * def u0 = (karate.call('@create_cookie').user)
    * def u1 = (karate.call('@create_cookie').user)
    * def u2 = (karate.call('@create_cookie').user)
    * def u3 = (karate.call('@create_cookie').user)
    * def data = 
    """
      ({
        t: u0.cookies.tid, s:u0.cookies.sid, e:u0.cookies.eid, 
        T: u1.cookies.tid, S:u2.cookies.sid, E:u3.cookies.eid
      })
    """

    @ignore @create_cookie
  Scenario: Create a consistent cookie set 
    Given def user = (karate.call('../utils/create_user.feature').user)
      * path '/auth/oauth2_flow/link'
      * params utils.defaultRedirects
      * param rememberMe = true
      * configure cookies = user.cookies
      * method get
      * status 200
      * match utils.getRedirectUrl(response) contains 'http://mock.localhost.com:8090/oauth2/authorize'
      * match responseCookies contains deep utils.matchAuthCookiesValidate
      * match responseCookies.eid contains {"max-age": #(utils.SESSION_SCOPE)} 
      * user['cookies']['eid'] = responseCookies.eid.value


        
