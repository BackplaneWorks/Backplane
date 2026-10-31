import Base

# A screenshot belongs to the page on show. A bot's browser uses its bot
# id; an ordinary thread uses the thread's page, with no bot selected.
def BrowserFrame.owns(+shown: String, source: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(shown)), String.eq(shown, source))

def BrowserFrame.owner(+bot: String, page: String) -> String:
  Bool.pick(String, String.is_empty(bot), page, bot)
