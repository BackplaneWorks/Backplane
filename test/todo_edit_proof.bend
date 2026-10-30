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
