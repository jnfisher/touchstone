"""Exceptions shared by the oracle's lexer, parser and interpreter."""


class VBSyntaxError(Exception):
    """The source is outside the VB6 subset the oracle understands."""

    def __init__(self, message, line=None, module=None):
        self.message = message
        self.line = line
        self.module = module
        loc = ""
        if module:
            loc += module + ":"
        if line is not None:
            loc += str(line) + ":"
        super().__init__((loc + " " if loc else "") + message)


class VBError(Exception):
    """A VB6 run-time error (catchable by On Error)."""

    def __init__(self, number, description=None):
        self.number = number
        self.description = description or ERROR_TEXT.get(number, "Application-defined or object-defined error")
        super().__init__("VB error %d: %s" % (number, self.description))


class OracleUnbound(Exception):
    """The program touched a name that is not loaded (form, recordset, other module).

    Deliberately not a VBError: On Error handlers must not swallow it, because a swallowed
    unbound reference would silently change a fixture.
    """

    def __init__(self, name, detail=None):
        self.name = name
        self.detail = detail
        super().__init__("unbound: %s%s" % (name, " (" + detail + ")" if detail else ""))


class ExitProc(Exception):
    pass


class EndProgram(Exception):
    """The VB `End` statement."""


ERROR_TEXT = {
    5: "Invalid procedure call or argument",
    6: "Overflow",
    9: "Subscript out of range",
    11: "Division by zero",
    13: "Type mismatch",
    91: "Object variable or With block variable not set",
    92: "For loop not initialized",
    94: "Invalid use of Null",
    424: "Object required",
    438: "Object doesn't support this property or method",
    448: "Named argument not found",
    449: "Argument not optional",
    450: "Wrong number of arguments or invalid property assignment",
    451: "Property let procedure not defined and property get procedure did not return an object",
    3: "Return without GoSub",
    20: "Resume without error",
    10: "This array is fixed or temporarily locked",
    35: "Sub or Function not defined",
}
