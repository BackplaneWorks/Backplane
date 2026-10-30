import Base
import ../src/core/todo.bend as TD
import ../src/core/model.bend as M

# Shared agent/person to-do list. A repeated field preserves the person's
# edit; an explicit different value wins. Completion clears at turn end.

law todo_person_item_survives_snapshot:
  for person: String
  {TD.Todo.merge([TD.Todo{"1", "Agent text", "", "pending"}], [TD.Todo{"1", person, "", "completed"}], [TD.Todo{"1", "Agent text", "", "pending"}])
    == [TD.Todo{"1", person, "", "completed"}] : List<&2, TD.Todo>}

law todo_person_delete_survives_snapshot:
  {TD.Todo.merge([TD.Todo{"1", "a", "", "pending"}], [], [TD.Todo{"1", "a", "", "pending"}]) == [] : List<&2, TD.Todo>}

law todo_agent_explicit_change:
  {TD.Todo.merge([TD.Todo{"1", "a", "", "pending"}], [TD.Todo{"1", "Person text", "", "completed"}], [TD.Todo{"1", "Agent text", "", "in_progress"}])
    == [TD.Todo{"1", "Agent text", "", "in_progress"}] : List<&2, TD.Todo>}

law todo_complete_auto_clears:
  {M.Setting.get(M.State.apply(M.State.apply(M.State.empty(), M.TodosSet{"t", "[{\"id\":\"1\",\"text\":\"a\",\"status\":\"completed\"}]", 1n}), M.TurnChanged{"t", M.Completed{}, 2n}), "todos.t", "") == "" : String}

law todo_next_turn_hides_old:
  {M.Todos.visible(M.State.apply(M.State.apply(M.State.empty(), M.TodosSet{"t", "[{\"id\":\"1\",\"text\":\"a\",\"status\":\"pending\"}]", 1n}), M.TurnChanged{"t", M.Running{}, 2n}), "t") == False{} : Bool}

# A list that stayed hidden all turn (the agent wrote none, the person did
# not edit it) is retired at the turn's end: current and merge base empty.
law todo_hidden_turn_retires:
  {List.append(&2, TD.Todo, M.Todos.of(M.State.apply(M.State.apply(M.State.apply(M.State.empty(), M.TodosSet{"t", "[{\"id\":\"1\",\"text\":\"a\",\"status\":\"pending\"}]", 1n}), M.TurnChanged{"t", M.Running{}, 2n}), M.TurnChanged{"t", M.Completed{}, 3n}), "t"),
    M.Todos.base(M.State.apply(M.State.apply(M.State.apply(M.State.empty(), M.TodosSet{"t", "[{\"id\":\"1\",\"text\":\"a\",\"status\":\"pending\"}]", 1n}), M.TurnChanged{"t", M.Running{}, 2n}), M.TurnChanged{"t", M.Completed{}, 3n}), "t")) == [] : List<&2, TD.Todo>}

# After a retire, the agent's next list is exactly its own: the person's
# edits and additions to the retired list do not come back.
law todo_write_after_retire_fresh:
  {M.Todos.of(M.State.apply(M.State.apply(M.State.apply(M.State.apply(M.State.apply(M.State.apply(M.State.empty(),
      M.TodosSet{"t", "[{\"id\":\"1\",\"text\":\"a\",\"status\":\"pending\"},{\"id\":\"2\",\"text\":\"b\",\"status\":\"pending\"}]", 1n}),
      M.TodosChanged{"t", "person", TD.Todo.ask("t", "set", "2", "Person", "", 0n), 2n}),
      M.TodosChanged{"t", "person", TD.Todo.ask("t", "add", "h:1", "Extra", "", 0n), 3n}),
      M.TurnChanged{"t", M.Running{}, 4n}), M.TurnChanged{"t", M.Completed{}, 5n}),
      M.TodosChanged{"t", "tools", "[{\"type\":\"tool_use\",\"name\":\"TodoWrite\",\"input\":{\"todos\":[{\"content\":\"a\",\"status\":\"in_progress\"},{\"content\":\"b\",\"status\":\"pending\"}]}}]", 6n}), "t")
    == [TD.Todo{"1", "a", "", "in_progress"}, TD.Todo{"2", "b", "", "pending"}] : List<&2, TD.Todo>}


def todo_person_item_survives_snapshot(person):
  {==}

def todo_person_delete_survives_snapshot():
  {==}

def todo_agent_explicit_change():
  {==}

def todo_complete_auto_clears():
  {==}

def todo_next_turn_hides_old():
  {==}

def todo_hidden_turn_retires():
  {==}

def todo_write_after_retire_fresh():
  {==}
