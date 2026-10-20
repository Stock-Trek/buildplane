variable "name" {
  description = "Queue name"
  type        = string
}

variable "role" {
  description = "Role to attach send message permission to"
  type        = string
}

variable "tags" {
  description = "Tags"
  type        = map(string)
  nullable    = true
  default     = {}
}

variable "fifo" {
  description = "Whether this should be a FIFO queue"
  type        = bool
  default     = false
}

variable "deduplication" {
  description = "Deduplication strategy. Only valid on a FIFO queue"
  type = object({
    mode  = string           # "none" | "content" | "explicit"
    scope = optional(string) # "queue" | "messageGroup"
  })
  default = { mode = "none" }

  validation {
    condition     = contains(["none", "content", "explicit"], var.deduplication.mode)
    error_message = "mode must be one of ['none', 'content', 'explicit']."
  }

  validation {
    condition = (
      var.deduplication.scope == null ||
      try(contains(["queue", "messageGroup"], var.deduplication.scope), false)
    )
    error_message = "scope must be one of [null, 'queue', 'messageGroup']."
  }
}

variable "dead_letter" {
  description = "Optional dead letter queue configuration"
  type = object({
    max_receive_count = number
  })
  nullable = true
  default  = null
}

variable "visibility_timeout" {
  description = "How long in seconds to wait for acknowledgement/deletion before the message is visible again"
  type        = number
  default     = 30
}
