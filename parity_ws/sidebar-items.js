initSidebarItems({"enum":[["CloseCode","Status code used to indicate why an endpoint is closing the WebSocket connection."],["ErrorKind","The type of an error, which may indicate other kinds of errors as the underlying cause."],["Message","An enum representing the various forms of a WebSocket message."],["OpCode","Operation codes as part of rfc6455."]],"fn":[["connect","A utility function for setting up a WebSocket client."],["listen","A utility function for setting up a WebSocket server."]],"mod":[["util","The util module rexports some tools from mio in order to facilitate handling timeouts."]],"struct":[["Builder","Utility for constructing a WebSocket from various settings."],["Error","A struct indicating the kind of error that has occurred and any precise details of that error."],["Frame","A struct representing a WebSocket frame."],["Handshake","A struct representing the two halves of the WebSocket handshake."],["Request","The handshake request."],["Response","The handshake response."],["Sender","A representation of the output of the WebSocket connection. Use this to send messages to the other endpoint."],["Settings","WebSocket settings"],["WebSocket","The WebSocket struct. A WebSocket can support multiple incoming and outgoing connections."]],"trait":[["Factory","A trait for creating new WebSocket handlers."],["Handler","The core trait of this library. Implementing this trait provides the business logic of the WebSocket application."]],"type":[["Result",""]]});